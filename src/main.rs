extern crate ansi_term;
#[cfg(test)]
extern crate assert_cli;
extern crate atty;
extern crate chrono;
extern crate chrono_english;
extern crate clap;
extern crate crossbeam_channel;
extern crate failure;
extern crate failure_derive;
#[cfg(test)]
extern crate indoc;
extern crate log;
extern crate regex;
extern crate stderrlog;
extern crate sysconf;
extern crate tabwriter;

mod commands;
mod parser;
mod proces;

use ansi_term::Color::*;
use ansi_term::Style;
use chrono::{DateTime, Local, TimeZone};
use chrono_english::{parse_date_string, Dialect};
use clap::{
    crate_version, App, AppSettings, Arg, ArgMatches, Error as ClapError, ErrorKind, SubCommand, value_t,
};
use failure::Error;
use failure_derive::Fail;
use log::*;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tabwriter::TabWriter;

use crate::commands::*;

fn main() {
    let arg_limit = Arg::with_name("limit")
        .long("limit")
        .takes_value(true)
        .default_value("10")
        .help("Use the last N merge times to predict next merge time.");
    let arg_pkg = Arg::with_name("package")
        .takes_value(true)
        .help("Show only packages matching <package>.");
    let arg_exact = Arg::with_name("exact")
        .short("e")
        .long("exact")
        .help("Match package with a string instead of a regex.")
        .long_help("Match package with a string instead of a regex. \
Regex is case-insensitive and matches on category/name (see https://docs.rs/regex/1.0.5/regex/#syntax). \
String is case-sentitive and matches on whole name, or whole category/name if it contains a /.");//FIXME auto crate version
    let arg_show_l = Arg::with_name("show")
        .short("s")
        .long("show")
        .value_name("m,s,a")
        .validator(|s| find_invalid("msa", &s))
        .default_value("m")
        .help("Show (m)erges, (s)yncs, and/or (a)ll.")
        .long_help("Show individual (m)erges, portage tree (s)yncs, or (a)ll of these (any letters combination).");
    let arg_show_s = Arg::with_name("show")
        .short("s")
        .long("show")
        .value_name("m,t,s,a")
        .validator(|s| find_invalid("msta", &s))
        .default_value("m")
        .help("Show (m)erges, (t)otals, (s)yncs, and/or (a)ll.")
        .long_help("Show individual (m)erges, (t)otal merges, portage tree (s)yncs, or (a)ll of these (any letters combination).");
    let arg_group = Arg::with_name("group")
        .short("g")
        .long("groupby")
        .value_name("y,m,w,d")
        .possible_values(&["y","m","w","d"])
        .hide_possible_values(true)
        .help("Group by (y)ear, (m)onth, (w)eek, or (d)ay.")
        .long_help("Group by (y)ear, (m)onth, (w)eek, or (d)ay.\n\
The grouping key is displayed in the first column. Weeks start on monday and are formated as 'year-weeknumber'.");
    let args = App::new("emlop")
        .version(crate_version!())
        .global_setting(AppSettings::ColoredHelp)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .global_setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::VersionlessSubcommands)
        .about("A fast, accurate, ergonomic EMerge LOg Parser.\nhttps://github.com/vincentdephily/emlop")
        .after_help("Subcommands can be abbreviated down to a single letter.\n\
Exit code is 0 if sucessful, 1 in case of errors (bad argument...), 2 if search found nothing.")
        .help_message("Show short (-h) or detailed (--help) help. Use <subcommand> -h/--help for subcommand help.")
        .arg(Arg::with_name("logfile")
             .value_name("file")
             .long("logfile")
             .short("f")
             .global(true)
             .takes_value(true)
             .default_value("/var/log/emerge.log")
             .help("Location of emerge log file."))
        .arg(Arg::with_name("from")
             .value_name("date")
             .long("from")
             .global(true)
             .takes_value(true)
             .help("Only parse log entries after <date>.")
             .long_help("Only parse log entries after <date>.\n\
Accepts string like '2018-03-04', '2018-03-04 12:34:56', 'march', '1 month ago', '10d ago', and unix timestamps... \
(see https://docs.rs/chrono-english/0.1.3/chrono_english/#supported-formats)."))
        .arg(Arg::with_name("to")
             .value_name("date")
             .long("to")
             .global(true)
             .takes_value(true)
             .help("Only parse log entries before <date>."))
        .arg(Arg::with_name("duration")
             .value_name("hms,s")
             .long("duration")
             .global(true)
             .possible_values(&["hms","s"])
             .hide_possible_values(true)
             .default_value("hms")
             .help("Format durations in hours:minutes:seconds or in seconds."))
        .arg(Arg::with_name("verbose")
             .short("v")
             .global(true)
             .multiple(true)
             .help("Show warnings (-v), info (-vv) and debug (-vvv) messages (errors are always displayed)."))
        .arg(Arg::with_name("color")
             .long("color").alias("colour")
             .global(true)
             .takes_value(true)
             .possible_values(&["auto","always","never","y","n"])
             .hide_possible_values(true)
             .default_value("auto")
             .value_name("when")
             .help("Enable color (auto/always/never/y/n)."))
        .subcommand(SubCommand::with_name("log")
                    .alias("list")
                    .about("Show log of sucessful merges and syncs.")
                    .long_about("Show log of sucessful merges and syncs.\n\
* Merges: date, duration, package name-version.\n\
* Syncs:  date, duration.")
                    .help_message("Show short (-h) or detailed (--help) help.")
                    .arg(&arg_show_l)
                    .arg(&arg_exact)
                    .arg(&arg_pkg))
        .subcommand(SubCommand::with_name("predict")
                    .about("Predict merge time for current or pretended merges.")
                    .long_about("Predict merge time for current or pretended merges.\n\
* If input is a terminal, predict time for the current merge (if any).\n\
* If input is a pipe (for example by running `emerge -rOp|emlop p`), predict time for those merges.")
                    .help_message("Show short (-h) or detailed (--help) help.")
                    .arg(&arg_limit))
        .subcommand(SubCommand::with_name("stats")
                    .about("Show statistics about sucessful merges and syncs.")
                    .long_about("Show statistics about sucessful merges (total or per package) merges and syncs.\n\
* Per-package: total merge time, total merge count, next merge time prediction.\n\
* Merges:      total merge time, total merge count, average merge time\n\
* Syncs:       total sync time,  total sync count,  average sync time")
                    .help_message("Show short (-h) or detailed (--help) help.")
                    .arg(&arg_show_s)
                    .arg(&arg_group)
                    .arg(&arg_exact)
                    .arg(&arg_pkg)
                    .arg(&arg_limit))
        .get_matches();

    stderrlog::new().verbosity(args.occurrences_of("verbose") as usize).init().unwrap();
    debug!("{:?}", args);
    let styles = Styles::new(&args);
    let mut tw = TabWriter::new(stdout());
    let res = match args.subcommand() {
        ("log",     Some(sub_args)) => cmd_list(&args, sub_args, &styles),
        ("stats",   Some(sub_args)) => cmd_stats(&mut tw, &args, sub_args, &styles),
        ("predict", Some(sub_args)) => cmd_predict(&mut tw, &args, sub_args, &styles),
        (other, _) => unimplemented!("{} subcommand", other),
    };
    tw.flush().unwrap_or(());
    match res {
        Ok(true) => ::std::process::exit(0),
        Ok(false) => ::std::process::exit(2),
        Err(e) => {
            error!("{}", e);
            ::std::process::exit(1)
        }
    }
}

/// Parse and return argument from an ArgMatches, exit if parsing fails.
///
/// This is similar to clap's `value_t!` except it takes a parsing function instead of a target
/// type, returns an unwraped value, and exits upon parsing error. It'd be more idiomatic to
/// implement FromStr trait on a custom struct, but this is simpler to write and use, and we're not
/// writing a library.
pub fn value<T,P>(matches: &ArgMatches, name: &str, parse: P) -> T
    where P: FnOnce(&str) -> Result<T,String> {
    match matches.value_of(name) {
        None => // Argument should be required by ArgMatch => this is a bug not a user error => panic
            panic!("Argument {} missing", name),
        Some(s) =>
            match parse(s) {
                Ok(v) => v,
                Err(e) => ClapError{message: format!("Invalid argument '--{} {}': {}", name, s, e),
                                    kind: ErrorKind::InvalidValue,
                                    info: None}.exit(),
            },
    }
}

/// Parse and return optional argument from an ArgMatches, exit if parsing fails.
///
/// See [value(m,n,p)->T] for background info.
///
/// [value(m,n,p)->T]:      fn.value.html
pub fn value_opt<T,P>(matches: &ArgMatches, name: &str, parse: P) -> Option<T>
    where P: FnOnce(&str) -> Result<T,String> {
    match matches.value_of(name) {
        None =>
            None,
        Some(s) =>
            match parse(s) {
                Ok(v) => Some(v),
                Err(e) => ClapError{message: format!("Invalid argument '--{} {}': {}", name, s, e),
                                    kind: ErrorKind::InvalidValue,
                                    info: None}.exit(),
            },
    }
}

pub fn parse_limit(s: &str) -> Result<u16, String> {
    u16::from_str(&s).map_err(|_| format!("Must be an integer between {} and {}", std::u16::MIN, std::u16::MAX))
}

pub fn parse_date(s: &str) -> Result<i64, String> {
    parse_date_string(s, Local::now(), Dialect::Uk)
        .map(|d| d.timestamp())
        .or_else(|_| i64::from_str(&s.trim()))
        .map_err(|_| "Couldn't parse as a date or timestamp".into())
}

#[derive(Debug)]
pub enum Timespan {
    Year,
    Month,
    Week,
    Day,
}
pub fn parse_timespan(s: &str) -> Result<Timespan, String> {
    match s {
        "y" => Ok(Timespan::Year),
        "m" => Ok(Timespan::Month),
        "w" => Ok(Timespan::Week),
        "d" => Ok(Timespan::Day),
        _ => Err("Valid values are y(ear), m(onth), w(eek), d(ay)".into()),
    }
}

/// Clap validation helper that checks that all chars are valid.
fn find_invalid(valid: &'static str, s: &str) -> Result<(), String> {
    debug_assert!(valid.is_ascii()); // Because we use `chars()` we need to stick to ascii for `valid`.
    match s.chars().find(|&c| !(valid.contains(c))) {
        None => Ok(()),
        Some(p) => Err(p.to_string()),
    }
}

pub enum DurationStyle {
    HMS,
    S,
}
impl FromStr for DurationStyle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hms" => Ok(DurationStyle::HMS),
            "s" => Ok(DurationStyle::S),
            _ => Err("Valid values are 'hms', 's'.".into())
        }
    }
}
#[rustfmt::skip]
pub fn fmt_duration(style: &DurationStyle, secs: i64) -> String {
    match style {
        DurationStyle::HMS => {
            let neg = if secs < 0 { "-" } else { "" };
            let h = (secs / 3600).abs();
            let m = (secs % 3600 / 60).abs();
            let s = (secs % 60).abs();
            if h > 0      { format!("{}{}:{:02}:{:02}", neg, h, m, s) }
            else if m > 0 { format!(      "{}{}:{:02}", neg, m, s) }
            else          { format!(            "{}{}", neg, s) }
        },
        DurationStyle::S => {
            format!("{}", secs)
        }
    }
}

pub fn fmt_time(ts: i64) -> DateTime<Local> {
    Local.timestamp(ts, 0)
}

pub fn epoch(st: SystemTime) -> i64 {
    st.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

pub fn epoch_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

/// Holds styling preferences (currently just color).
///
/// We're using prefix/suffix() instead of paint() because paint() doesn't handle '{:>9}' alignments
/// properly.
pub struct Styles {
    pkg_p: String,
    pkg_s: String,
    dur_p: String,
    dur_s: String,
    cnt_p: String,
    cnt_s: String,
}
impl Styles {
    fn new(args: &ArgMatches) -> Self {
        let enabled = match args.value_of("color") {
            Some("always") | Some("y") => true,
            Some("never") | Some("n") => false,
            _ => atty::is(atty::Stream::Stdout),
        };
        if enabled {
            Styles{pkg_p: Style::new().fg(Green).bold().prefix().to_string(),
                   pkg_s: Style::new().fg(Green).bold().suffix().to_string(),
                   dur_p: Style::new().fg(Purple).bold().prefix().to_string(),
                   dur_s: Style::new().fg(Purple).bold().suffix().to_string(),
                   cnt_p: Style::new().fg(Yellow).dimmed().prefix().to_string(),
                   cnt_s: Style::new().fg(Yellow).dimmed().suffix().to_string(),
            }
        } else {
            Styles{pkg_p: String::new(),
                   pkg_s: String::new(),
                   dur_p: String::new(),
                   dur_s: String::new(),
                   cnt_p: String::new(),
                   cnt_s: String::new(),
            }
        }
    }
}

#[derive(Debug, Fail)]
#[fail(display = "Cannot open {}: {}", file, reason)]
struct OpenError {
    file: String,
    reason: std::io::Error,
}
/// File::open wrapper with a more user-friendly error.
pub fn myopen(fname: &str) -> Result<impl Read, Error> {
    File::open(fname).map_err(|e| OpenError{file:fname.into(), reason: e}.into())
}


#[cfg(test)]
mod tests {
    use crate::*;

    #[test] #[rustfmt::skip]
    fn duration() {
        assert_eq!(        "0", fmt_duration(&DurationStyle::HMS, 0));
        assert_eq!(        "1", fmt_duration(&DurationStyle::HMS, 1));
        assert_eq!(       "59", fmt_duration(&DurationStyle::HMS, 59));
        assert_eq!(     "1:00", fmt_duration(&DurationStyle::HMS, 60));
        assert_eq!(     "1:01", fmt_duration(&DurationStyle::HMS, 61));
        assert_eq!(    "59:59", fmt_duration(&DurationStyle::HMS, 3599));
        assert_eq!(  "1:00:00", fmt_duration(&DurationStyle::HMS, 3600));
        assert_eq!( "99:59:59", fmt_duration(&DurationStyle::HMS, 359999));
        assert_eq!("100:00:00", fmt_duration(&DurationStyle::HMS, 360000));
        assert_eq!(       "-1", fmt_duration(&DurationStyle::HMS, -1));
        assert_eq!(    "-1:00", fmt_duration(&DurationStyle::HMS, -60));
        assert_eq!( "-1:00:00", fmt_duration(&DurationStyle::HMS, -3600));
    }

    #[test]
    fn date() {
        // Mainly testing the unix fallback here, as the rest is chrono_english's responsibility
        let now = epoch_now();
        assert_eq!(Ok(1522710000), parse_date("1522710000"));
        assert_eq!(Ok(1522710000), parse_date("   1522710000   "));
        assert_eq!(Ok(1522713661), parse_date("2018-04-03 01:01:01"));
        assert_eq!(Ok(now),        parse_date("now"));
        assert_eq!(Ok(now),        parse_date("   now   "));
        assert_eq!(Ok(now-3600),   parse_date("1 hour ago"));
        assert!(parse_date("03/30/18").is_err()); // MM/DD/YY is horrible, sorry USA
        assert!(parse_date("30/03/18").is_ok());  // DD/MM/YY is also bad, switch to YYYY-MM-DD already ;)
        assert!(parse_date("").is_err());
        assert!(parse_date("152271000o").is_err());
        assert!(parse_date("a while ago").is_err());
    }
}
