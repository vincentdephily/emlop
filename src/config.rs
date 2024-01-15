mod toml;

use crate::{*, config::toml::Toml};
use anyhow::{Error};
use clap::{error::{ContextKind, ContextValue, Error as ClapError, ErrorKind},
           ArgMatches};
use std::{env::var};


pub fn err(val: String, src: &'static str, possible: &'static str) -> ClapError {
    let mut err = clap::Error::new(ErrorKind::InvalidValue);
    err.insert(ContextKind::InvalidValue, ContextValue::String(val));
    let p = possible.split_ascii_whitespace().map(|s| s.to_string()).collect();
    err.insert(ContextKind::ValidValue, ContextValue::Strings(p));
    err.insert(ContextKind::InvalidArg, ContextValue::String(src.to_string()));
    err
}

pub trait ArgParse<T> {
    fn parse(val: &T, src: &'static str) -> Result<Self, ClapError>
        where Self: Sized;
}
impl ArgParse<bool> for bool {
    fn parse(b: &bool, _src: &'static str) -> Result<Self, ClapError> {
        Ok(*b)
    }
}
impl ArgParse<String> for bool {
    fn parse(s: &String, src: &'static str) -> Result<Self, ClapError> {
        match s.as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Err(err(s.to_owned(), src, "y(es) n(o)")),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub enum Average {
    Arith,
    #[default]
    Median,
    WeightedArith,
    WeightedMedian,
}
impl ArgParse<String> for Average {
    fn parse(s: &String, src: &'static str) -> Result<Self, ClapError> {
        use Average::*;
        match s.as_str() {
            "a" | "arith" => Ok(Arith),
            "m" | "median" => Ok(Median),
            "wa" | "weighted-arith" => Ok(WeightedArith),
            "wm" | "weighted-median" => Ok(WeightedMedian),
            _ => Err(err(s.to_owned(), src, "arith median weightedarith weigtedmedian a m wa wm")),
        }
    }
}

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
}
pub struct ConfLog {
    pub starttime: bool,
    pub first: usize,
}
pub struct ConfPred {
    pub avg: Average,
}
pub struct ConfStats {
    pub avg: Average,
}
pub struct ConfAccuracy {
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
fn sel<T, R>(args: &ArgMatches,
             argsrc: &'static str,
             toml: Option<&T>,
             tomlsrc: &'static str)
             -> Result<R, ClapError>
    where R: ArgParse<String> + ArgParse<T> + Default
{
    if let Some(a) = args.get_one::<String>(argsrc) {
        R::parse(a, argsrc)
    } else if let Some(a) = toml {
        R::parse(a, tomlsrc)
    } else {
        Ok(R::default())
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
        let date_fmt = sel(args, "date", toml.date.as_ref(), "date")?;
        let date_offset = get_offset(args.get_flag("utc"));
        Ok(Self { pkg: AnsiStr::from(if color { "\x1B[1;32m" } else { "" }),
                  merge: AnsiStr::from(if color { "\x1B[1;32m" } else { ">>> " }),
                  unmerge: AnsiStr::from(if color { "\x1B[1;31m" } else { "<<< " }),
                  dur: AnsiStr::from(if color { "\x1B[1;35m" } else { "" }),
                  cnt: AnsiStr::from(if color { "\x1B[2;33m" } else { "" }),
                  clr: AnsiStr::from(if color { "\x1B[0m" } else { "" }),
                  lineend: if color { b"\x1B[0m\n" } else { b"\n" },
                  header,
                  dur_t,
                  date_offset,
                  date_fmt,
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
        Ok(Self { starttime: sel(args,
                                 "starttime",
                                 toml.log.as_ref().and_then(|l| l.starttime.as_ref()),
                                 "[log] starttime")?,
                  first: *args.get_one("first").unwrap_or(&usize::MAX) })
    }
}

impl ConfPred {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { avg: sel(args,
                           "avg",
                           toml.predict.as_ref().and_then(|t| t.average.as_ref()),
                           "[predict] average")? })
    }
}

impl ConfStats {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { avg: sel(args,
                           "avg",
                           toml.stats.as_ref().and_then(|t| t.average.as_ref()),
                           "[predict] average")? })
    }
}
impl ConfAccuracy {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { avg: sel(args,
                           "avg",
                           toml.accuracy.as_ref().and_then(|t| t.average.as_ref()),
                           "[predict] average")? })
    }
}
