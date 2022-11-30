mod cli;
mod commands;
mod date;
mod parse;
mod proces;
mod table;

use crate::{commands::*, date::*};
use anyhow::Error;
use clap::{ArgMatches, Command, ErrorKind};
use log::*;
use std::str::FromStr;
use time::UtcOffset;

fn main() {
    let args = cli::build_cli().get_matches();
    let level = match args.occurrences_of("verbose") {
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
        Ok(true) => ::std::process::exit(0),
        Ok(false) => ::std::process::exit(1),
        Err(e) => {
            match e.source() {
                Some(s) => error!("{}: {}", e, s),
                None => error!("{}", e),
            }
            ::std::process::exit(2)
        },
    }
}

/// Parse and return argument from an ArgMatches, exit if parsing fails.
///
/// This is the same as [value_opt(m,n,p)->Option<T>] except that we expect `name` to have a
/// value. Note the nice exit for user error vs panic for emlop bug.
///
/// [value_opt(m,n,p)->Option<T>]: fn.value_opt.html
pub fn value<T, P>(matches: &ArgMatches, name: &str, parse: P) -> T
    where P: FnOnce(&str) -> Result<T, String>
{
    let s = matches.value_of(name).unwrap_or_else(|| panic!("Argument {} missing", name));
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

#[derive(Clone, Copy)]
pub enum Average {
    Mean,
    Median,
    Weighted,
}
impl Average {
    fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "mean" => Ok(Self::Mean),
            "median" => Ok(Self::Median),
            "weighted" => Ok(Self::Weighted),
            _ => Err("Valid values are 'mean', 'median', 'weighted'."),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResumeKind {
    Auto,
    Main,
    Backup,
    No,
}
impl ResumeKind {
    fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            _ if "auto".starts_with(s) => Ok(Self::Auto),
            _ if "main".starts_with(s) => Ok(Self::Main),
            _ if "backup".starts_with(s) => Ok(Self::Backup),
            _ if "no".starts_with(s) => Ok(Self::No),
            _ => Err("Valid values are 'auto', 'main', 'backup', 'no' (can be abbreviated)."),
        }
    }
}

#[derive(Clone, Copy)]
pub enum DurationStyle {
    HMS,
    HMSFixed,
    Secs,
    Human,
}
impl DurationStyle {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "hms" => Ok(Self::HMS),
            "hms_fixed" => Ok(Self::HMSFixed),
            "s" => Ok(Self::Secs),
            "human" => Ok(Self::Human),
            _ => Err("Valid values are 'hms', 'hms_fixed', 's', 'human'.".into()),
        }
    }

    fn fmt(&self, secs: i64) -> String {
        if secs < 0 {
            return String::from("?");
        }
        match &self {
            Self::HMS if secs >= 3600 => {
                format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
            },
            Self::HMS if secs >= 60 => format!("{}:{:02}", secs % 3600 / 60, secs % 60),
            Self::HMS => format!("{}", secs),
            Self::HMSFixed => {
                format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
            },
            Self::Human if secs == 0 => String::from("0 second"),
            Self::Human => {
                let mut buf = String::with_capacity(16);
                Self::fmt_append(&mut buf, secs / 86400, "day");
                Self::fmt_append(&mut buf, secs % 86400 / 3600, "hour");
                Self::fmt_append(&mut buf, secs % 3600 / 60, "minute");
                Self::fmt_append(&mut buf, secs % 60, "second");
                buf
            },
            Self::Secs => format!("{}", secs),
        }
    }

    fn fmt_append(buf: &mut String, num: i64, what: &'static str) {
        use std::fmt::Write;

        if num == 0 {
            return;
        }
        let prefix = if buf.is_empty() { "" } else { ", " };
        if num == 1 {
            write!(buf, "{prefix}{num} {what}").expect("write to string");
        } else {
            write!(buf, "{prefix}{num} {what}s").expect("write to string");
        }
    }
}

/// Holds styling preferences.
///
/// Colors use prefix/suffix() instead of paint() because paint() doesn't handle '{:>9}' alignments
/// properly.
pub struct Styles {
    pkg: &'static str,
    merge: &'static str,
    unmerge: &'static str,
    dur: &'static str,
    cnt: &'static str,
    clr: &'static str,
    header: bool,
    dur_t: DurationStyle,
    date_offset: UtcOffset,
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
        let dur_fmt = *args.get_one("duration").unwrap();
        let date_fmt = args.value_of_t("date").unwrap();
        let utc = args.get_flag("utc");
        let tabs = args.get_flag("tabs");
        Styles::new(color, header, dur_fmt, date_fmt, utc, tabs)
    }

    fn new(color: bool,
           header: bool,
           duration: DurationStyle,
           date: DateStyle,
           utc: bool,
           tabs: bool)
           -> Self {
        if color {
            Styles { pkg: "\x1B[1;32m",
                     merge: "\x1B[1;32m",
                     unmerge: "\x1B[1;31m",
                     dur: "\x1B[1;35m",
                     cnt: "\x1B[2;33m",
                     clr: "\x1B[0m",
                     header,
                     dur_t: duration,
                     date_offset: date::get_offset(utc),
                     date_fmt: date,
                     tabs }
        } else {
            Styles { pkg: "",
                     merge: ">>> ",
                     unmerge: "<<< ",
                     dur: "",
                     cnt: "",
                     clr: "",
                     header,
                     dur_t: duration,
                     date_offset: date::get_offset(utc),
                     date_fmt: date,
                     tabs }
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn duration() {
        for (hms, hms_fixed, secs, human, i) in
            [("0", "0:00:00", "0", "0 second", 0),
             ("1", "0:00:01", "1", "1 second", 1),
             ("59", "0:00:59", "59", "59 seconds", 59),
             ("1:00", "0:01:00", "60", "1 minute", 60),
             ("1:01", "0:01:01", "61", "1 minute, 1 second", 61),
             ("59:59", "0:59:59", "3599", "59 minutes, 59 seconds", 3599),
             ("1:00:00", "1:00:00", "3600", "1 hour", 3600),
             ("48:00:01", "48:00:01", "172801", "2 days, 1 second", 172801),
             ("99:59:59", "99:59:59", "359999", "4 days, 3 hours, 59 minutes, 59 seconds", 359999),
             ("100:00:00", "100:00:00", "360000", "4 days, 4 hours", 360000),
             ("?", "?", "?", "?", -1),
             ("?", "?", "?", "?", -123456)]
        {
            assert_eq!(hms, DurationStyle::HMS.fmt(i));
            assert_eq!(hms_fixed, DurationStyle::HMSFixed.fmt(i));
            assert_eq!(human, DurationStyle::Human.fmt(i));
            assert_eq!(secs, DurationStyle::Secs.fmt(i));
        }
    }
}
