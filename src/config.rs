/// Runtime config
///
/// Order of precedance is command line (clap), config file (toml), default.
mod cli;
mod toml;
mod types;

pub use crate::config::{cli::*, types::*};
use crate::{config::toml::Toml, parse::AnsiStr, *};
use clap::ArgMatches;
use std::{io::IsTerminal, path::PathBuf};

/// Global config, one enum variant per command
pub enum Configs {
    Log(Conf, ConfLog),
    Stats(Conf, ConfStats),
    Predict(Conf, ConfPred),
    Accuracy(Conf, ConfAccuracy),
    Complete(Conf, ConfComplete),
}
/// Common config
///
/// Using raw `set/clear` ANSI colors instead of some `paint()` method to simplify alignment.
pub struct Conf {
    pub pkg: AnsiStr,
    pub merge: AnsiStr,
    pub unmerge: AnsiStr,
    pub dur: AnsiStr,
    pub cnt: AnsiStr,
    pub skip: AnsiStr,
    pub clr: AnsiStr,
    pub lineend: &'static [u8],
    pub header: bool,
    pub showskip: bool,
    pub dur_t: DurationStyle,
    pub date_offset: time::UtcOffset,
    pub date_fmt: DateStyle,
    pub out: OutStyle,
    pub logfile: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
}
pub struct ConfLog {
    pub show: Show,
    pub search: Vec<String>,
    pub exact: bool,
    pub starttime: bool,
    pub first: usize,
    pub last: usize,
}
pub struct ConfPred {
    pub show: Show,
    pub avg: Average,
    pub first: usize,
    pub last: usize,
    pub lim: u16,
    pub resume: ResumeKind,
    pub unknown: i64,
    pub tmpdirs: Vec<PathBuf>,
    pub pwidth: usize,
    pub pdepth: usize,
}
pub struct ConfStats {
    pub show: Show,
    pub search: Vec<String>,
    pub exact: bool,
    pub avg: Average,
    pub lim: u16,
    pub group: Timespan,
}
pub struct ConfAccuracy {
    pub show: Show,
    pub search: Vec<String>,
    pub exact: bool,
    pub avg: Average,
    pub last: usize,
    pub lim: u16,
}
pub struct ConfComplete {
    #[cfg(feature = "clap_complete")]
    pub shell: Option<String>,
    pub pkg: Option<String>,
}

impl Configs {
    pub fn load() -> Result<Configs, Error> {
        let cli = cli::build_cli().get_matches();
        let level = match cli.get_count("verbose") {
            0 => LevelFilter::Error,
            1 => LevelFilter::Warn,
            2 => LevelFilter::Info,
            3 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        };
        env_logger::Builder::new().filter_level(level).format_timestamp(None).init();
        trace!("{:?}", cli);
        let toml = Toml::load()?;
        trace!("{:?}", toml);
        let conf = Conf::try_new(&cli, &toml)?;
        Ok(match cli.subcommand() {
            Some(("log", sub)) => Self::Log(conf, ConfLog::try_new(sub, &toml)?),
            Some(("stats", sub)) => Self::Stats(conf, ConfStats::try_new(sub, &toml)?),
            Some(("predict", sub)) => Self::Predict(conf, ConfPred::try_new(sub, &toml)?),
            Some(("accuracy", sub)) => Self::Accuracy(conf, ConfAccuracy::try_new(sub, &toml)?),
            Some(("complete", sub)) => Self::Complete(conf, ConfComplete::try_new(sub)?),
            _ => unreachable!("clap should have exited already"),
        })
    }
}

// TODO nicer way to specify src
fn sel<T, A, R>(cli: Option<&String>,
                toml: Option<&T>,
                clisrc: &'static str,
                tomlsrc: &'static str,
                arg: A,
                def: R)
                -> Result<R, ArgError>
    where R: ArgParse<String, A> + ArgParse<T, A>
{
    if let Some(a) = cli {
        R::parse(a, arg, clisrc)
    } else if let Some(a) = toml {
        R::parse(a, arg, tomlsrc)
    } else {
        Ok(def)
    }
}

macro_rules! sel {
    ($cli: expr, $toml: expr, $name: ident, $arg: expr, $def: expr) => {
        sel($cli.get_one::<String>(stringify!($name)),
            $toml.$name.as_ref(),
            concat!("--", stringify!($name)),
            stringify!($name),
            $arg,
            $def)
    };
    ($cli: expr, $toml: expr, $section: ident, $name: ident, $arg: expr, $def: expr) => {
        sel($cli.get_one::<String>(stringify!($name)),
            $toml.$section.as_ref().and_then(|t| t.$name.as_ref()),
            concat!("--", stringify!($name)),
            stringify!([$section] $name),
            $arg,
            $def)
    }
}

impl Conf {
    pub fn try_new(cli: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        let isterm = std::io::stdout().is_terminal();
        let color = sel!(cli, toml, color, isterm, isterm)?;
        let outdef = if isterm { OutStyle::Columns } else { OutStyle::Tab };
        let offset = get_offset(sel!(cli, toml, utc, (), false)?);
        Ok(Self { logfile: sel!(cli, toml, logfile, (), String::from("/var/log/emerge.log"))?,
                  from: cli.get_one("from")
                           .map(|d| i64::parse(d, offset, "--from"))
                           .transpose()?,
                  to: cli.get_one("to").map(|d| i64::parse(d, offset, "--to")).transpose()?,
                  pkg: AnsiStr::from(if color { "\x1B[1;32m" } else { "" }),
                  merge: AnsiStr::from(if color { "\x1B[1;32m" } else { ">>> " }),
                  unmerge: AnsiStr::from(if color { "\x1B[1;31m" } else { "<<< " }),
                  dur: AnsiStr::from(if color { "\x1B[1;35m" } else { "" }),
                  skip: AnsiStr::from(if color { "\x1B[37m" } else { "" }),
                  cnt: AnsiStr::from(if color { "\x1B[33m" } else { "" }),
                  clr: AnsiStr::from(if color { "\x1B[m" } else { "" }),
                  lineend: if color { b"\x1B[m\n" } else { b"\n" },
                  header: sel!(cli, toml, header, (), false)?,
                  showskip: sel!(cli, toml, showskip, (), true)?,
                  dur_t: sel!(cli, toml, duration, (), DurationStyle::Hms)?,
                  date_offset: offset,
                  date_fmt: sel!(cli, toml, date, (), DateStyle::default())?,
                  out: sel!(cli, toml, output, isterm, outdef)? })
    }
    #[cfg(test)]
    pub fn from_str(s: impl AsRef<str>) -> Self {
        let cli = cli::build_cli().get_matches_from(s.as_ref().split_whitespace());
        Self::try_new(&cli, &Toml::default()).unwrap()
    }
}

impl ConfLog {
    fn try_new(cli: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(cli, toml, log, show, "musa", Show::m())?,
                  search: cli.get_many("search").unwrap_or_default().cloned().collect(),
                  exact: cli.get_flag("exact"),
                  starttime: sel!(cli, toml, log, starttime, (), false)?,
                  first: *cli.get_one("first").unwrap_or(&usize::MAX),
                  last: *cli.get_one("last").unwrap_or(&usize::MAX) })
    }
}

impl ConfPred {
    fn try_new(cli: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        let tmpdirs = if let Some(a) = cli.get_many::<PathBuf>("tmpdir") {
            a.cloned().collect()
        } else if let Some(a) = toml.predict.as_ref().and_then(|t| t.tmpdir.as_ref()) {
            a.to_vec()
        } else {
            vec![PathBuf::from("/var/tmp")]
        };
        Ok(Self { show: sel!(cli, toml, predict, show, "emta", Show::emt())?,
                  avg: sel!(cli, toml, predict, avg, (), Average::Median)?,
                  lim: sel!(cli, toml, predict, limit, 1..=65000, 10)? as u16,
                  unknown: sel!(cli, toml, predict, unknown, 0..=3600, 10)?,
                  resume: *cli.get_one("resume").unwrap_or(&ResumeKind::Auto),
                  tmpdirs,
                  first: *cli.get_one("first").unwrap_or(&usize::MAX),
                  last: *cli.get_one("last").unwrap_or(&usize::MAX),
                  pwidth: sel!(cli, toml, predict, pwidth, 10..=1000, 60)? as usize,
                  pdepth: sel!(cli, toml, predict, pdepth, 0..=100, 3)? as usize })
    }
    #[cfg(test)]
    pub fn from_str(s: impl AsRef<str>) -> (Conf, Self) {
        let cli = cli::build_cli().get_matches_from(s.as_ref().split_whitespace());
        (Conf::try_new(&cli, &Toml::default()).unwrap(),
         ConfPred::try_new(cli.subcommand().unwrap().1, &Toml::default()).unwrap())
    }
}

impl ConfStats {
    fn try_new(cli: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(cli, toml, stats, show, "ptsa", Show::p())?,
                  search: cli.get_many("search").unwrap_or_default().cloned().collect(),
                  exact: cli.get_flag("exact"),
                  lim: sel!(cli, toml, stats, limit, 1..=65000, 10)? as u16,
                  avg: sel!(cli, toml, stats, avg, (), Average::Median)?,
                  group: sel!(cli, toml, stats, group, (), Timespan::None)? })
    }
}

impl ConfAccuracy {
    fn try_new(cli: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(cli, toml, accuracy, show, "mta", Show::mt())?,
                  search: cli.get_many("search").unwrap_or_default().cloned().collect(),
                  exact: cli.get_flag("exact"),
                  avg: sel!(cli, toml, accuracy, avg, (), Average::Median)?,
                  lim: sel!(cli, toml, accuracy, limit, 1..=65000, 10)? as u16,
                  last: *cli.get_one("last").unwrap_or(&usize::MAX) })
    }
}

impl ConfComplete {
    fn try_new(cli: &ArgMatches) -> Result<Self, Error> {
        Ok(Self { #[cfg(feature = "clap_complete")]
                  shell: cli.get_one("shell").cloned(),
                  pkg: cli.get_one("pkg").cloned() })
    }
}
