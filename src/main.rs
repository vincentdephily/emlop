#[cfg(test)]
extern crate assert_cli;
extern crate atty;
extern crate chrono;
#[macro_use]
extern crate clap;
#[cfg(test)]
#[macro_use]
extern crate indoc;
extern crate regex;
extern crate sysconf;
extern crate tabwriter;

mod commands;
mod parser;
mod proces;

use chrono::{DateTime, Local, TimeZone};
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use std::io;
use std::io::Write;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tabwriter::TabWriter;

use commands::*;

fn main() {
    let arg_limit = Arg::with_name("limit")
        .long("limit")
        .takes_value(true)
        .default_value("10")
        .validator(is_posint)
        .help("Use the last N merge times to predict next merge time.");
    let arg_pkg = Arg::with_name("package")
        .takes_value(true)
        .help("Display only packages matching <package>.");
    let arg_exact = Arg::with_name("exact")
        .short("e")
        .long("exact")
        .help("Match package with a string instead of a regex.")
        .long_help("Match package with a string instead of a regex. \
Regex is case-insensitive and matches on category/name (see https://docs.rs/regex/0.2.6/regex/index.html#syntax). \
String is case-sentitive and matches on whole name, or whole category/name if it contains a /.");//FIXME auto crate version
    let args = App::new("emlop")
        .version(crate_version!())
        .global_setting(AppSettings::ColoredHelp)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::VersionlessSubcommands)
        .about("A fast, accurate, ergonnomic EMerge LOg Parser.\nhttps://github.com/vincentdephily/emlop")
        .after_help("Subcommands can be abbreviated down to a single letter.")
        .help_message("Prints help information. Use --help for more details. Use <subcommand> -h for subcommand help.")
        .arg(Arg::with_name("logfile")
             .long("logfile")
             .short("f")
             .global(true)
             .takes_value(true)
             .default_value("/var/log/emerge.log")
             .help("Location of emerge log file."))
        .arg(Arg::with_name("mindate")
             .long("from")
             .global(true)
             .takes_value(true)
             .help("Only consider events from that date onward.")
             .long_help("Only consider events from that date onward.\n\
Currently accepted format is a unix timestamp, get one using `$(date -d 'human-readable date' +%s)`."))
        .arg(Arg::with_name("maxdate")
             .long("to")
             .global(true)
             .takes_value(true)
             .help("Only consider events up to that date.")
             .long_help("Only consider events up to that date.\n\
Currently accepted format is a unix timestamp, get one using `$(date -d 'human-readable date' +%s)`."))
        .subcommand(SubCommand::with_name("list")
                    .about("Show list of completed merges.")
                    .long_about("Show list of completed merges.\n\
Merge date, merge time, package name-version.")
                    .help_message("Prints help information. Use --help for more details.")
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

    let mut tw = TabWriter::new(io::stdout());
    match args.subcommand() {
        ("list",    Some(sub_args)) => cmd_list(&args, sub_args),
        ("stats",   Some(sub_args)) => cmd_stats(&mut tw, &args, sub_args),
        ("predict", Some(sub_args)) => cmd_predict(&mut tw, &args, sub_args),
        (other, _) => unimplemented!("{} subcommand", other),
    }.unwrap();
    tw.flush().unwrap_or(());
}

fn is_posint(v: String) -> Result<(), String> {
    match u32::from_str(&v) {
        Ok(id) if id > 0 => Ok(()),
        _ => Err("Must be an positive integer.".into()),
    }
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
}
