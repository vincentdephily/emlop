use anyhow::{Context, Error};
use log::debug;
use serde::Deserialize;
use std::{env::var, fs::File, io::Read, path::PathBuf, time::Instant};

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
    pub unknownb: Option<i64>,
    pub unknownc: Option<i64>,
    pub tmpdir: Option<Vec<PathBuf>>,
    pub pwidth: Option<i64>,
    pub pdepth: Option<i64>,
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
    pub showskip: Option<bool>,
    pub utc: Option<bool>,
    pub color: Option<String>,
    pub output: Option<String>,
    pub theme: Option<String>,
    pub log: Option<TomlLog>,
    pub predict: Option<TomlPred>,
    pub stats: Option<TomlStats>,
    pub accuracy: Option<TomlAccuracy>,
}
impl Toml {
    pub fn load() -> Result<Self, Error> {
        let now = Instant::now();
        let (src, res) = match var("EMLOP_CONFIG").ok() {
            Some(s) if s.is_empty() => (String::from("default config"), Ok(Self::default())),
            Some(s) => {
                let r = Self::doload(&s);
                (s, r)
            },
            _ => {
                let s = format!("{}/.config/emlop.toml", var("HOME").unwrap_or("".to_string()));
                let r = Self::doload(&s);
                (s, r)
            },
        };
        debug!("Loaded {src} in {:?}", now.elapsed());
        res
    }
    fn doload(name: &str) -> Result<Self, Error> {
        match File::open(name) {
            Err(e) => {
                log::warn!("Cannot open {name:?}: {e}");
                Ok(Self::default())
            },
            Ok(mut f) => {
                let mut buf = String::new();
                f.read_to_string(&mut buf).with_context(|| format!("Cannot read {name:?}"))?;
                toml::from_str(&buf).with_context(|| format!("Cannot parse {name:?}"))
            },
        }
    }
}
