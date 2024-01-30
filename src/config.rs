mod toml;
mod types;

use crate::{config::toml::Toml, *};
use anyhow::Error;
use clap::ArgMatches;
use std::env::var;
pub use types::*;


pub enum Configs {
    Log(ArgMatches, Conf, ConfLog),
    Stats(ArgMatches, Conf, ConfStats),
    Predict(ArgMatches, Conf, ConfPred),
    Accuracy(ArgMatches, Conf, ConfAccuracy),
    Complete(ArgMatches),
}

/// Global config
///
/// Colors use `prefix/suffix()` instead of `paint()` because `paint()` doesn't handle `'{:>9}'`
/// alignments properly.
pub struct Conf {
    pub pkg: AnsiStr,
    pub merge: AnsiStr,
    pub unmerge: AnsiStr,
    pub dur: AnsiStr,
    pub cnt: AnsiStr,
    pub clr: AnsiStr,
    pub lineend: &'static [u8],
    pub header: bool,
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
    pub starttime: bool,
    pub first: usize,
}
pub struct ConfPred {
    pub show: Show,
    pub avg: Average,
}
pub struct ConfStats {
    pub show: Show,
    pub avg: Average,
}
pub struct ConfAccuracy {
    pub show: Show,
    pub avg: Average,
}

impl Configs {
    pub fn load() -> Result<Configs, Error> {
        let args = cli::build_cli().get_matches();
        let level = match args.get_count("verbose") {
            0 => LevelFilter::Error,
            1 => LevelFilter::Warn,
            2 => LevelFilter::Info,
            3 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        };
        env_logger::Builder::new().filter_level(level).format_timestamp(None).init();
        trace!("{:?}", args);
        let toml = Toml::load(args.get_one::<String>("config"), var("EMLOP_CONFIG").ok())?;
        log::trace!("{:?}", toml);
        let conf = Conf::try_new(&args, &toml)?;
        Ok(match args.subcommand() {
            Some(("log", sub)) => Self::Log(sub.clone(), conf, ConfLog::try_new(sub, &toml)?),
            Some(("stats", sub)) => Self::Stats(sub.clone(), conf, ConfStats::try_new(sub, &toml)?),
            Some(("predict", sub)) => {
                Self::Predict(sub.clone(), conf, ConfPred::try_new(sub, &toml)?)
            },
            Some(("accuracy", sub)) => {
                Self::Accuracy(sub.clone(), conf, ConfAccuracy::try_new(sub, &toml)?)
            },
            Some(("complete", sub)) => Self::Complete(sub.clone()),
            _ => unreachable!("clap should have exited already"),
        })
    }
}

// TODO nicer way to specify src
fn sel<T, A, R>(cli: Option<&String>,
                toml: Option<&T>,
                argsrc: &'static str,
                tomlsrc: &'static str,
                parg: A,
                def: R)
                -> Result<R, ArgError>
    where R: ArgParse<String, A> + ArgParse<T, A>
{
    if let Some(a) = cli {
        R::parse(a, parg, argsrc)
    } else if let Some(a) = toml {
        R::parse(a, parg, tomlsrc)
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
    pub fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        let isterm = std::io::stdout().is_terminal();
        let color = match args.get_one("color") {
            Some(ColorStyle::Always) => true,
            Some(ColorStyle::Never) => false,
            None => isterm,
        };
        let out = match args.get_one("output") {
            Some(o) => *o,
            None if isterm => OutStyle::Columns,
            None => OutStyle::Tab,
        };
        let header = args.get_flag("header");
        let dur_t = *args.get_one("duration").unwrap();
        let offset = get_offset(args.get_flag("utc"));
        Ok(Self { logfile: sel!(args, toml, logfile, (), String::from("/var/log/emerge.log"))?,
                  from: args.get_one("from")
                            .map(|d| i64::parse(d, offset, "--from"))
                            .transpose()?,
                  to: args.get_one("to").map(|d| i64::parse(d, offset, "--to")).transpose()?,
                  pkg: AnsiStr::from(if color { "\x1B[1;32m" } else { "" }),
                  merge: AnsiStr::from(if color { "\x1B[1;32m" } else { ">>> " }),
                  unmerge: AnsiStr::from(if color { "\x1B[1;31m" } else { "<<< " }),
                  dur: AnsiStr::from(if color { "\x1B[1;35m" } else { "" }),
                  cnt: AnsiStr::from(if color { "\x1B[2;33m" } else { "" }),
                  clr: AnsiStr::from(if color { "\x1B[0m" } else { "" }),
                  lineend: if color { b"\x1B[0m\n" } else { b"\n" },
                  header,
                  dur_t,
                  date_offset: offset,
                  date_fmt: sel!(args, toml, date, (), DateStyle::default())?,
                  out })
    }
    #[cfg(test)]
    pub fn from_str(s: impl AsRef<str>) -> Self {
        let args = cli::build_cli().get_matches_from(s.as_ref().split_whitespace());
        Self::try_new(&args, &Toml::default()).unwrap()
    }
}

impl ConfLog {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(args, toml, log, show, "musa", Show::m())?,
                  starttime: sel!(args, toml, log, starttime, (), false)?,
                  first: *args.get_one("first").unwrap_or(&usize::MAX) })
    }
}

impl ConfPred {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(args, toml, predict, show, "emta", Show::emt())?,
                  avg: sel!(args, toml, predict, avg, (), Average::Median)? })
    }
}

impl ConfStats {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(args, toml, stats, show, "ptsa", Show::p())?,
                  avg: sel!(args, toml, stats, avg, (), Average::Median)? })
    }
}
impl ConfAccuracy {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { show: sel!(args, toml, accuracy, show, "mta", Show::mt())?,
                  avg: sel!(args, toml, accuracy, avg, (), Average::Median)? })
    }
}
