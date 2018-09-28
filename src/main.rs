extern crate ansi_term;
#[cfg(test)] extern crate assert_cli;
extern crate atty;
extern crate chrono;
extern crate chrono_english;
#[macro_use] extern crate clap;
extern crate crossbeam_channel;
extern crate failure;
#[macro_use] extern crate failure_derive;
#[cfg(test)] #[macro_use] extern crate indoc;
#[macro_use] extern crate log;
extern crate regex;
extern crate stderrlog;
extern crate sysconf;
extern crate tabwriter;

mod commands;
mod parser;
mod proces;

use ansi_term::Style;
use ansi_term::Color::*;
use chrono::{DateTime, Local, TimeZone};
use chrono_english::{parse_date_string,Dialect};
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use failure::Error;
use std::fs::File;
use std::{io, io::{Read, Write}};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tabwriter::TabWriter;

use commands::*;

fn main() {
    let arg_limit = Arg::with_name("limit")
        .long("limit")
        .takes_value(true)
        .default_value("10")
        .help("Use the last N merge times to predict next merge time.");
    let arg_pkg = Arg::with_name("package")
        .takes_value(true)
        .help("Display only packages matching <package>.");
    let arg_exact = Arg::with_name("exact")
        .short("e")
        .long("exact")
        .help("Match package with a string instead of a regex.")
        .long_help("Match package with a string instead of a regex. \
Regex is case-insensitive and matches on category/name (see https://docs.rs/regex/1.0.5/regex/#syntax). \
String is case-sentitive and matches on whole name, or whole category/name if it contains a /.");//FIXME auto crate version
    let arg_type = Arg::with_name("types")
        .short("t")
        .long("types")
        .value_name("m,s")
        .possible_values(&["m","s"])
        .use_delimiter(true)
        .hide_possible_values(true)
        .default_value("m")
        .help("Show (m)erges, and/or (s)yncs.")
        .long_help("Show history of package (m)erges and/or portage tree (s)yncs (comma-delimited list).");
    let arg_sync = Arg::with_name("sync")
        .short("s")
        .long("sync")
        .conflicts_with("types")
        .help("Show only syncs.")
        .long_help("Show only history of portage tree sync (overrides --types).");
    let args = App::new("emlop")
        .version(crate_version!())
        .global_setting(AppSettings::ColoredHelp)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::VersionlessSubcommands)
        .about("A fast, accurate, ergonnomic EMerge LOg Parser.\nhttps://github.com/vincentdephily/emlop")
        .after_help("Subcommands can be abbreviated down to a single letter.\n\
Exit code is 0 if sucessful, 1 in case of errors (bad argument...), 2 if search found nothing.")
        .help_message("Prints help information. Use --help for more details. Use <subcommand> -h for subcommand help.")
        .arg(Arg::with_name("logfile")
             .value_name("path/to/file")
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
        .subcommand(SubCommand::with_name("list")
                    .about("Show list of completed merges.")
                    .long_about("Show list of completed merges.\n\
Merge date, merge time, package name-version.")
                    .help_message("Prints help information. Use --help for more details.")
                    .arg(&arg_type)
                    .arg(&arg_sync)
                    .arg(&arg_exact)
                    .arg(&arg_pkg))
        .subcommand(SubCommand::with_name("predict")
                    .about("Predict merge time for current or pretended merges.")
                    .long_about("Predict merge time for current or pretended merges.\n\
If input is a terminal, predict time for the current merge (if any).\n\
If input is a pipe (for example by running `emerge -rOp|emlop p`), predict time for those merges.")
                    .help_message("Prints help information. Use --help for more details.")
                    .arg(&arg_limit))
        .subcommand(SubCommand::with_name("stats")
                    .about("Show statistics for completed merges.")
                    .long_about("Show statistics for completed merges.\n\
Total merge time, total merge count, and next merge time prediction.")
                    .help_message("Prints help information. Use --help for more details.")
                    .arg(&arg_exact)
                    .arg(&arg_pkg)
                    .arg(&arg_limit))
        .get_matches();

    stderrlog::new().verbosity(args.occurrences_of("verbose") as usize).init().unwrap();
    debug!("{:?}", args);
    let styles = Styles::new(&args);
    let mut tw = TabWriter::new(io::stdout());
    let res = match args.subcommand() {
        ("list",    Some(sub_args)) => cmd_list(&args, sub_args, styles),
        ("stats",   Some(sub_args)) => cmd_stats(&mut tw, &args, sub_args, styles),
        ("predict", Some(sub_args)) => cmd_predict(&mut tw, &args, sub_args, styles),
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
    where P: FnOnce(&str) -> Result<T,String>, T: std::str::FromStr {
    match matches.value_of(name) {
        None => // Argument should be required by ArgMatch => this is a bug not a user error => panic
            panic!("Argument {} missing", name),
        Some(s) =>
            match parse(s) {
                Ok(v) => v,
                Err(e) => clap::Error{message: format!("Invalid argument '--{} {}': {}", name, s, e),
                                      kind: clap::ErrorKind::InvalidValue,
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
    where P: FnOnce(&str) -> Result<T,String>, T: std::str::FromStr {
    match matches.value_of(name) {
        None =>
            None,
        Some(s) =>
            match parse(s) {
                Ok(v) => Some(v),
                Err(e) => clap::Error{message: format!("Invalid argument '--{} {}': {}", name, s, e),
                                      kind: clap::ErrorKind::InvalidValue,
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

pub fn fmt_duration(secs: i64) -> String {
    let neg = if secs < 0 { "-" } else { "" };
    let h = (secs / 3600).abs();
    let m = (secs % 3600 / 60).abs();
    let s = (secs % 60).abs();
    if h > 0      { format!("{}{}:{:02}:{:02}", neg, h, m, s) }
    else if m > 0 { format!(      "{}{}:{:02}", neg, m, s) }
    else          { format!(            "{}{}", neg, s) }
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
    use ::*;

    #[test]
    fn duration() {
        assert_eq!(        "0", fmt_duration(0));
        assert_eq!(        "1", fmt_duration(1));
        assert_eq!(       "59", fmt_duration(59));
        assert_eq!(     "1:00", fmt_duration(60));
        assert_eq!(     "1:01", fmt_duration(61));
        assert_eq!(    "59:59", fmt_duration(3599));
        assert_eq!(  "1:00:00", fmt_duration(3600));
        assert_eq!( "99:59:59", fmt_duration(359999));
        assert_eq!("100:00:00", fmt_duration(360000));
        assert_eq!(       "-1", fmt_duration(-1));
        assert_eq!(    "-1:00", fmt_duration(-60));
        assert_eq!( "-1:00:00", fmt_duration(-3600));
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
