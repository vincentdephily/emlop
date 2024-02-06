use anyhow::{Context, Error};
use serde::Deserialize;
use std::{env::var, fs::File, io::Read, path::PathBuf};

#[derive(Deserialize, Debug)]
pub struct TomlLog {
    pub show: Option<String>,
    pub starttime: Option<bool>,
}
#[derive(Deserialize, Debug)]
pub struct TomlPred {
    pub show: Option<String>,
    pub avg: Option<String>,
    pub limit: Option<i64>,
    pub unknown: Option<i64>,
    pub tmpdir: Option<Vec<PathBuf>>,
}
#[derive(Deserialize, Debug)]
pub struct TomlStats {
    pub show: Option<String>,
    pub avg: Option<String>,
    pub limit: Option<i64>,
    pub group: Option<String>,
}
#[derive(Deserialize, Debug)]
pub struct TomlAccuracy {
    pub show: Option<String>,
    pub avg: Option<String>,
    pub limit: Option<i64>,
}
#[derive(Deserialize, Debug, Default)]
pub struct Toml {
    pub logfile: Option<String>,
    pub date: Option<String>,
    pub duration: Option<String>,
    pub header: Option<bool>,
    pub utc: Option<bool>,
    pub log: Option<TomlLog>,
    pub predict: Option<TomlPred>,
    pub stats: Option<TomlStats>,
    pub accuracy: Option<TomlAccuracy>,
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
        toml::from_str(&buf).with_context(|| format!("Cannot parse {name:?}"))
    }
}
