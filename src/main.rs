#![cfg_attr(feature = "unstable", feature(test))]

mod cli;
mod commands;
mod datetime;
mod parse;
mod proces;
mod table;

use crate::{commands::*, datetime::*, parse::AnsiStr};
use anyhow::Error;
use clap::{error::ErrorKind, ArgMatches, Error as ClapErr};
use log::*;
use std::{io::IsTerminal, str::FromStr};

fn main() {
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
    let res = match args.subcommand() {
        Some(("log", sub_args)) => cmd_list(sub_args),
        Some(("stats", sub_args)) => cmd_stats(sub_args),
        Some(("predict", sub_args)) => cmd_predict(sub_args),
        Some(("accuracy", sub_args)) => cmd_accuracy(sub_args),
        Some(("complete", sub_args)) => cmd_complete(sub_args),
        _ => unreachable!("clap should have exited already"),
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

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Average {
    #[clap(alias("a"))]
    Arith,
    #[clap(alias("m"))]
    Median,
    #[clap(alias("wa"))]
    WeightedArith,
    #[clap(alias("wm"))]
    WeightedMedian,
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

/// Holds styling preferences.
///
/// Colors use `prefix/suffix()` instead of `paint()` because `paint()` doesn't handle `'{:>9}'`
/// alignments properly.
pub struct Styles {
    pkg: AnsiStr,
    merge: AnsiStr,
    unmerge: AnsiStr,
    dur: AnsiStr,
    cnt: AnsiStr,
    clr: AnsiStr,
    lineend: &'static [u8],
    header: bool,
    dur_t: DurationStyle,
    date_offset: time::UtcOffset,
    date_fmt: DateStyle,
    out: OutStyle,
}
impl Styles {
    fn from_args(args: &ArgMatches) -> Self {
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
        let date_fmt = *args.get_one("date").unwrap();
        let date_offset = get_offset(args.get_flag("utc"));
        Styles { pkg: AnsiStr::from(if color { "\x1B[1;32m" } else { "" }),
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
                 out }
    }
    #[cfg(test)]
    fn from_str(s: impl AsRef<str>) -> Self {
        let args = cli::build_cli().get_matches_from(s.as_ref().split_whitespace());
        Self::from_args(&args)
    }
}
