#![cfg_attr(feature = "unstable", feature(test))]

mod cli;
mod commands;
mod config;
mod datetime;
mod parse;
mod proces;
mod table;

use crate::{commands::*, config::*, datetime::*, parse::AnsiStr};
use anyhow::Error;
use clap::{error::ErrorKind, ArgMatches, Error as ClapErr};
use log::*;
use std::{io::IsTerminal, str::FromStr};

fn main() {
    let res = match Configs::load() {
        Ok(Configs::Log(args, conf, sconf)) => cmd_log(&args, &conf, sconf),
        Ok(Configs::Stats(args, conf, sconf)) => cmd_stats(&args, &conf, sconf),
        Ok(Configs::Predict(args, conf, sconf)) => cmd_predict(&args, &conf, sconf),
        Ok(Configs::Accuracy(args, conf, sconf)) => cmd_accuracy(&args, &conf, sconf),
        Ok(Configs::Complete(args)) => cmd_complete(&args),
        Err(e) => Err(e),
    };
    match res {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(e) => match e.downcast::<ClapErr>() {
            Ok(ce) => ce.format(&mut cli::build_cli()).exit(),
            Err(e) => {
                log_err(e);
                std::process::exit(2)
            },
        },
    }
}

pub fn log_err(e: Error) {
    match e.source() {
        Some(s) => error!("{}: {}", e, s),
        None => error!("{}", e),
    }
}

/// Parse and return optional argument from an ArgMatches
///
/// This is similar to clap's `get_one()` with `value_parser` except it allows late parsing with an
/// argument.
pub fn get_parse<T, P, A>(args: &ArgMatches,
                          name: &str,
                          parse: P,
                          arg: A)
                          -> Result<Option<T>, ClapErr>
    where P: FnOnce(&str, A) -> Result<T, &'static str>
{
    match args.get_one::<String>(name) {
        None => Ok(None),
        Some(s) => match parse(s, arg) {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(ClapErr::raw(ErrorKind::InvalidValue,
                                       format!("\"{s}\" isn't a valid for '--{name}': {e}"))),
        },
    }
}

/// Alias to `write!(...).expect("write to buf")` just to save on typing/indent.
#[macro_export]
macro_rules! wtb { ($b:ident, $($arg:expr),+) => {write!($b, $($arg),+).expect("write to buf")} }

#[derive(Clone, Copy, Default)]
pub struct Show {
    pub pkg: bool,
    pub tot: bool,
    pub sync: bool,
    pub merge: bool,
    pub unmerge: bool,
    pub emerge: bool,
}
impl Show {
    fn parse(show: &str, valid: &str) -> Result<Self, String> {
        debug_assert!(valid.is_ascii()); // Because we use `chars()` we need to stick to ascii for `valid`.
        if show.chars().all(|c| valid.contains(c)) {
            Ok(Self { pkg: show.contains('p') || show.contains('a'),
                      tot: show.contains('t') || show.contains('a'),
                      sync: show.contains('s') || show.contains('a'),
                      merge: show.contains('m') || show.contains('a'),
                      unmerge: show.contains('u') || show.contains('a'),
                      emerge: show.contains('e') || show.contains('a') })
        } else {
            Err(format!("Valid values are letters of '{valid}'"))
        }
    }
}
impl std::fmt::Display for Show {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sep = "";
        for (b, s) in [(self.pkg, "pkg"),
                       (self.tot, "total"),
                       (self.sync, "sync"),
                       (self.merge, "merge"),
                       (self.unmerge, "unmerge"),
                       (self.emerge, "emerge")]
        {
            if b {
                write!(f, "{sep}{s}")?;
                sep = ",";
            }
        }
        Ok(())
    }
}


#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ResumeKind {
    #[clap(alias("a"))]
    Any,
    #[clap(alias("m"))]
    Main,
    #[clap(alias("b"))]
    Backup,
    #[clap(alias("n"))]
    No,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum DurationStyle {
    HMS,
    #[clap(id("hmsfixed"))]
    HMSFixed,
    #[clap(alias("s"))]
    Secs,
    #[clap(alias("h"))]
    Human,
}

#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ColorStyle {
    #[clap(alias("y"))]
    Always,
    #[clap(alias("n"))]
    Never,
}

#[derive(Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum OutStyle {
    #[clap(alias("c"))]
    Columns,
    #[clap(alias("t"))]
    Tab,
}
