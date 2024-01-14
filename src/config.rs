use crate::DateStyle;
use anyhow::{Context, Error};
use clap::{error::{ContextKind, ContextValue, Error as ClapError, ErrorKind},
           ArgMatches};
use serde::Deserialize;
use std::{env::var, fs::File, io::Read};

#[derive(Deserialize, Debug, Default)]
struct TomlLog {
    starttime: Option<bool>,
}
#[derive(Deserialize, Debug, Default)]
struct TomlPred {
    average: Option<String>,
}
#[derive(Deserialize, Debug, Default)]
struct TomlStats {
    average: Option<String>,
}
#[derive(Deserialize, Debug, Default)]
struct TomlAccuracy {
    average: Option<String>,
}
#[derive(Deserialize, Debug, Default)]
pub struct Toml {
    date: Option<String>,
    log: Option<TomlLog>,
    predict: Option<TomlPred>,
    stats: Option<TomlStats>,
    accuracy: Option<TomlAccuracy>,
}
impl Toml {
    fn load(arg: Option<&String>, env: Option<String>) -> Result<Self, Error> {
        match arg.or(env.as_ref()) {
            Some(s) if s.is_empty() => Ok(Self::default()),
            Some(s) => Self::doload(s.as_str()),
            _ => Self::doload(&format!("{}/.config/emlop.toml",
                                       var("HOME").unwrap_or("".to_string()))),
        }
    }
    fn doload(name: &str) -> Result<Self, Error> {
        log::trace!("Loading config {name:?}");
        let mut f = File::open(name).with_context(|| format!("Cannot open {name:?}"))?;
        let mut buf = String::new();
        // TODO Streaming read
        f.read_to_string(&mut buf).with_context(|| format!("Cannot read {name:?}"))?;
        toml::from_str(&buf).with_context(|| format!("Cannot parse {name:?}"))
    }
}

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

pub enum Config<'a> {
    Log(&'a ArgMatches, ConfigAll, ConfigLog),
    Stats(&'a ArgMatches, ConfigAll, ConfigStats),
    Predict(&'a ArgMatches, ConfigAll, ConfigPred),
    Accuracy(&'a ArgMatches, ConfigAll, ConfigAccuracy),
    Complete(&'a ArgMatches),
}
pub struct ConfigAll {
    pub date: DateStyle,
}
pub struct ConfigLog {
    pub starttime: bool,
    pub first: usize,
}
pub struct ConfigPred {
    pub avg: Average,
}
pub struct ConfigStats {
    pub avg: Average,
}
pub struct ConfigAccuracy {
    pub avg: Average,
}

impl<'a> Config<'a> {
    pub fn try_new(args: &'a ArgMatches) -> Result<Self, Error> {
        let toml = Toml::load(args.get_one::<String>("config"), var("EMLOP_CONFIG").ok())?;
        log::trace!("{:?}", toml);
        let conf = ConfigAll::try_new(args, &toml)?;
        Ok(match args.subcommand() {
            Some(("log", sub)) => Self::Log(sub, conf, ConfigLog::try_new(sub, &toml)?),
            Some(("stats", sub)) => Self::Stats(sub, conf, ConfigStats::try_new(sub, &toml)?),
            Some(("predict", sub)) => Self::Predict(sub, conf, ConfigPred::try_new(sub, &toml)?),
            Some(("accuracy", sub)) => {
                Self::Accuracy(sub, conf, ConfigAccuracy::try_new(sub, &toml)?)
            },
            Some(("complete", sub)) => Self::Complete(sub),
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

impl ConfigAll {
    pub fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { date: sel(args, "date", toml.date.as_ref(), "date")? })
    }
}

impl ConfigLog {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { starttime: sel(args,
                                 "starttime",
                                 toml.log.as_ref().and_then(|l| l.starttime.as_ref()),
                                 "[log] starttime")?,
                  first: *args.get_one("first").unwrap_or(&usize::MAX) })
    }
}

impl ConfigPred {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { avg: sel(args,
                           "avg",
                           toml.predict.as_ref().and_then(|t| t.average.as_ref()),
                           "[predict] average")? })
    }
}

impl ConfigStats {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { avg: sel(args,
                           "avg",
                           toml.stats.as_ref().and_then(|t| t.average.as_ref()),
                           "[predict] average")? })
    }
}
impl ConfigAccuracy {
    fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { avg: sel(args,
                           "avg",
                           toml.accuracy.as_ref().and_then(|t| t.average.as_ref()),
                           "[predict] average")? })
    }
}
