use crate::DateStyle;
use anyhow::{Context, Error};
use clap::{error::{ContextKind, ContextValue, Error as ClapError},
           ArgMatches};
use serde::Deserialize;
use std::{env::var, fs::File, io::Read};

#[derive(Deserialize, Debug, Default)]
pub struct Toml {
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

fn err_src(mut err: ClapError, src: String) -> ClapError {
    err.insert(ContextKind::InvalidArg, ContextValue::String(src));
    err
}
fn select<T>(arg: Option<&String>,
             argsrc: &'static str,
             toml: &Option<String>,
             tomlsrc: &'static str,
             def: &'static str)
             -> Result<T, ClapError>
    where T: for<'a> TryFrom<&'a str, Error = ClapError>
{
    if let Some(a) = arg {
        T::try_from(a.as_str()).map_err(|e| err_src(e, format!("{argsrc} (argument)")))
    } else if let Some(a) = toml {
        T::try_from(a.as_str()).map_err(|e| err_src(e, format!("{tomlsrc} (config)")))
    } else {
        Ok(T::try_from(def).expect("default value"))
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
        Ok(Self { date: select(args.get_one("date"), "--date", &toml.date, "date", "ymdhms")? })
    }
}

impl ConfigLog {
    pub fn try_new(args: &ArgMatches, _toml: &Toml) -> Result<Self, Error> {
        Ok(Self { first: *args.get_one("first").unwrap_or(&usize::MAX) })
    }
}
