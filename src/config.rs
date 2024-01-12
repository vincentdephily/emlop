use crate::DateStyle;
use anyhow::{Context, Error};
use clap::{error::{ContextKind, ContextValue, Error as ClapError, ErrorKind},
           ArgMatches};
use serde::Deserialize;
use std::{env::var, fs::File, io::Read};

#[derive(Deserialize, Debug, Default, Clone, Copy)]
pub struct TomlLog {
    starttime: Option<bool>,
}
#[derive(Deserialize, Debug, Default)]
pub struct Toml {
    log: Option<TomlLog>,
    date: Option<String>,
}
impl Toml {
    pub fn load(arg: Option<&String>, env: Option<String>) -> Result<Self, Error> {
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
        Ok(toml::from_str(&buf).with_context(|| format!("Cannot parse {name:?}"))?)
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

// TODO nicer way to specify src
fn sel<A, B, T>(arg: Option<&A>,
                argsrc: &'static str,
                toml: &Option<B>,
                tomlsrc: &'static str)
                -> Result<T, ClapError>
    where T: ArgParse<A> + ArgParse<B> + Default
{
    if let Some(a) = arg {
        T::parse(a, argsrc)
    } else if let Some(a) = toml {
        T::parse(a, tomlsrc)
    } else {
        Ok(T::default())
    }
}

pub enum Config<'a> {
    Log(&'a ArgMatches, ConfigAll, ConfigLog),
    Stats(&'a ArgMatches, ConfigAll),
    Predict(&'a ArgMatches, ConfigAll),
    Accuracy(&'a ArgMatches, ConfigAll),
    Complete(&'a ArgMatches),
}
pub struct ConfigAll {
    pub date: DateStyle,
}
pub struct ConfigLog {
    pub starttime: bool,
    pub first: usize,
}

impl<'a> Config<'a> {
    pub fn try_new(args: &'a ArgMatches) -> Result<Self, Error> {
        let toml = Toml::load(args.get_one::<String>("config"), var("EMLOP_CONFIG").ok())?;
        log::trace!("{:?}", toml);
        let conf = ConfigAll::try_new(&args, &toml)?;
        Ok(match args.subcommand() {
            Some(("log", sub)) => Self::Log(sub, conf, ConfigLog::try_new(sub, &toml)?),
            Some(("stats", sub)) => Self::Stats(sub, conf),
            Some(("predict", sub)) => Self::Predict(sub, conf),
            Some(("accuracy", sub)) => Self::Accuracy(sub, conf),
            Some(("complete", sub)) => Self::Complete(sub),
            _ => unreachable!("clap should have exited already"),
        })
    }
}

impl ConfigAll {
    pub fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { date: sel(args.get_one("date"), "--date", &toml.date, "date")? })
    }
}

impl ConfigLog {
    pub fn try_new(args: &ArgMatches, toml: &Toml) -> Result<Self, Error> {
        Ok(Self { starttime: sel(args.get_one::<String>("starttime"),
                                 "--starttime",
                                 &toml.log.and_then(|l| l.starttime),
                                 "[log] starttime")?,
                  first: *args.get_one("first").unwrap_or(&usize::MAX) })
    }
}
