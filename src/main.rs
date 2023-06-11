mod cli;
mod commands;
mod datetime;
mod parse;
mod proces;
mod table;

use crate::{commands::*, datetime::*, parse::AnsiStr};
use anyhow::Error;
use clap::{ArgMatches, Command, ErrorKind};
use log::*;
use std::str::FromStr;

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
    debug!("{:?}", args);
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
        Err(e) => {
            log_err(e);
            std::process::exit(2)
        },
    }
}

pub fn log_err(e: Error) {
    match e.source() {
        Some(s) => error!("{}: {}", e, s),
        None => error!("{}", e),
    }
}

/// Parse and return argument from an ArgMatches, exit if parsing fails.
///
/// This is the same as [`value_opt(m,n,p)->Option<T>`] except that we expect `name` to have a
/// value. Note the nice exit for user error vs panic for emlop bug.
///
/// [`value_opt(m,n,p)->Option<T>`]: fn.value_opt.html
pub fn value<T, P>(matches: &ArgMatches, name: &str, parse: P) -> T
    where P: FnOnce(&str) -> Result<T, String>
{
    let s = matches.value_of(name).unwrap_or_else(|| panic!("Argument {name} missing"));
    match parse(s) {
        Ok(v) => v,
        Err(e) => Command::new("emlop").error(ErrorKind::InvalidValue,
                                              format!("Invalid argument '--{name} {s}': {e}"))
                                       .exit(),
    }
}

/// Parse and return optional argument from an ArgMatches, exit if parsing fails.
///
/// This is similar to clap's `value_t!` except it takes a parsing function instead of a target
/// type, returns an unwraped value, and exits upon parsing error. It'd be more idiomatic to
/// implement FromStr trait on a custom struct, but this is simpler to write and use, and we're not
/// writing a library.
pub fn value_opt<T, P, A>(matches: &ArgMatches, name: &str, parse: P, arg: A) -> Option<T>
    where P: FnOnce(&str, A) -> Result<T, String>
{
    let s = matches.value_of(name)?;
    match parse(s, arg) {
        Ok(v) => Some(v),
        Err(e) => Command::new("emlop").error(ErrorKind::InvalidValue,
                                              format!("Invalid argument '--{name} {s}': {e}"))
                                       .exit(),
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
    Auto,
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
    tabs: bool,
}
impl Styles {
    fn from_args(args: &ArgMatches) -> Self {
        let color = match args.value_of("color") {
            Some("always") | Some("y") => true,
            Some("never") | Some("n") => false,
            _ => atty::is(atty::Stream::Stdout),
        };
        let header = args.get_flag("header");
        let dur_t = *args.get_one("duration").unwrap();
        let date_fmt = args.value_of_t("date").unwrap();
        let date_offset = get_offset(args.get_flag("utc"));
        let tabs = args.get_flag("tabs");
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
                 tabs }
    }
    #[cfg(test)]
    fn from_str(s: impl AsRef<str>) -> Self {
        let args = cli::build_cli().get_matches_from(s.as_ref().split_whitespace());
        Self::from_args(&args)
    }
}
