mod cli;
mod commands;
mod date;
mod parser;
mod proces;
mod table;

use crate::{commands::*, date::*};
use ansi_term::{Color::*, Style};
use anyhow::Error;
use clap::{value_t, ArgMatches, Error as ClapError, ErrorKind};
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
    let styles = Styles::from_args(&args);
    let res = match args.subcommand() {
        ("log", Some(sub_args)) => cmd_list(&args, sub_args, &styles),
        ("stats", Some(sub_args)) => cmd_stats(&args, sub_args, &styles),
        ("predict", Some(sub_args)) => cmd_predict(&args, sub_args, &styles),
        ("complete", Some(sub_args)) => cmd_complete(sub_args),
        (other, _) => unimplemented!("{} subcommand", other),
    };
    match res {
        Ok(true) => ::std::process::exit(0),
        Ok(false) => ::std::process::exit(2),
        Err(e) => {
            match e.source() {
                Some(s) => error!("{}: {}", e, s),
                None => error!("{}", e),
            }
            ::std::process::exit(1)
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
        Err(e) => ClapError { message: format!("Invalid argument '--{} {}': {}", name, s, e),
                              kind: ErrorKind::InvalidValue,
                              info: None }.exit(),
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
        Err(e) => ClapError { message: format!("Invalid argument '--{} {}': {}", name, s, e),
                              kind: ErrorKind::InvalidValue,
                              info: None }.exit(),
    }
}

pub fn parse_limit(s: &str) -> Result<u16, String> {
    u16::from_str(s).map_err(|_| {
                        format!("Must be an integer between {} and {}",
                                std::u16::MIN,
                                std::u16::MAX)
                    })
}


#[derive(Clone, Copy, Default)]
pub struct Show {
    pub pkg: bool,
    pub tot: bool,
    pub sync: bool,
    pub merge: bool,
    pub unmerge: bool,
}
impl FromStr for Show {
    type Err = String;
    fn from_str(show: &str) -> Result<Self, Self::Err> {
        Ok(Self { pkg: show.contains(&"p") || show.contains(&"a"),
                  tot: show.contains(&"t") || show.contains(&"a"),
                  sync: show.contains(&"s") || show.contains(&"a"),
                  merge: show.contains(&"m") || show.contains(&"a"),
                  unmerge: show.contains(&"u") || show.contains(&"a") })
    }
}

#[derive(Clone, Copy)]
pub enum DurationStyle {
    HMS,
    HMSFixed,
    Secs,
    Human,
}
impl FromStr for DurationStyle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hms" => Ok(DurationStyle::HMS),
            "hms_fixed" => Ok(DurationStyle::HMSFixed),
            "s" => Ok(DurationStyle::Secs),
            "human" => Ok(DurationStyle::Human),
            _ => Err("Valid values are 'hms', 'hms_fixed', 's'.".into()),
        }
    }
}
pub fn fmt_duration(style: DurationStyle, secs: i64) -> String {
    if secs < 0 {
        return String::from("?");
    }
    match style {
        DurationStyle::HMS if secs >= 3600 => {
            format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
        },
        DurationStyle::HMS if secs >= 60 => format!("{}:{:02}", secs % 3600 / 60, secs % 60),
        DurationStyle::HMS => format!("{}", secs),
        DurationStyle::HMSFixed => {
            format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
        },
        DurationStyle::Human if secs == 0 => String::from("0 second"),
        DurationStyle::Human => {
            let mut buf = String::with_capacity(16);
            fmt_duration_append(&mut buf, secs / 86400, "day");
            fmt_duration_append(&mut buf, secs % 86400 / 3600, "hour");
            fmt_duration_append(&mut buf, secs % 3600 / 60, "minute");
            fmt_duration_append(&mut buf, secs % 60, "second");
            buf
        },
        DurationStyle::Secs => format!("{}", secs),
    }
}
fn fmt_duration_append(buf: &mut String, num: i64, what: &'static str) {
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

/// Holds styling preferences.
///
/// Colors use prefix/suffix() instead of paint() because paint() doesn't handle '{:>9}' alignments
/// properly.
pub struct Styles {
    pkg_p: String,
    merge_p: String,
    merge_s: String,
    unmerge_p: String,
    dur_p: String,
    dur_s: String,
    cnt_p: String,
    cnt_s: String,
    dur_t: DurationStyle,
    date_offset: UtcOffset,
    date_fmt: DateStyle,
}
impl Styles {
    fn from_args(args: &ArgMatches) -> Self {
        let color = match args.value_of("color") {
            Some("always") | Some("y") => true,
            Some("never") | Some("n") => false,
            _ => atty::is(atty::Stream::Stdout),
        };
        let dur_fmt = value_t!(args, "duration", DurationStyle).unwrap();
        let date_fmt = value_t!(args, "date", DateStyle).unwrap();
        let utc = args.is_present("utc");
        Styles::new(color, dur_fmt, date_fmt, utc)
    }

    fn new(color: bool, duration: DurationStyle, date: DateStyle, utc: bool) -> Self {
        if color {
            Styles { pkg_p: Style::new().fg(Green).bold().prefix().to_string(),
                     merge_p: Style::new().fg(Green).bold().prefix().to_string(),
                     merge_s: Style::new().fg(Green).bold().suffix().to_string(),
                     unmerge_p: Style::new().fg(Red).bold().prefix().to_string(),
                     dur_p: Style::new().fg(Purple).bold().prefix().to_string(),
                     dur_s: Style::new().fg(Purple).bold().suffix().to_string(),
                     cnt_p: Style::new().fg(Yellow).dimmed().prefix().to_string(),
                     cnt_s: Style::new().fg(Yellow).dimmed().suffix().to_string(),
                     dur_t: duration,
                     date_offset: date::get_offset(utc),
                     date_fmt: date }
        } else {
            Styles { pkg_p: String::new(),
                     merge_p: String::from(">>> "),
                     merge_s: String::new(),
                     unmerge_p: String::from("<<< "),
                     dur_p: String::new(),
                     dur_s: String::new(),
                     cnt_p: String::new(),
                     cnt_s: String::new(),
                     dur_t: duration,
                     date_offset: date::get_offset(utc),
                     date_fmt: date }
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
            assert_eq!(hms, fmt_duration(DurationStyle::HMS, i));
            assert_eq!(hms_fixed, fmt_duration(DurationStyle::HMSFixed, i));
            assert_eq!(human, fmt_duration(DurationStyle::Human, i));
            assert_eq!(secs, fmt_duration(DurationStyle::Secs, i));
        }
    }
}
