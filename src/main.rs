extern crate atty;
extern crate chrono;
#[macro_use]
extern crate clap;
extern crate regex;
extern crate sysconf;
extern crate tabwriter;

mod commands;
mod parser;
mod proces;

use chrono::{DateTime, Local, TimeZone};
use clap::{AppSettings, Arg, ArgMatches, SubCommand};
use std::io;
use std::io::Write;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tabwriter::TabWriter;

use commands::*;

fn main() {
    let arg_limit = Arg::with_name("limit")
        .long("limit")
        .short("l")
        .takes_value(true)
        .default_value("10")
        .validator(is_posint)
        .help("Use the last N merge times to predict future merge time");
    let arg_pkg = Arg::with_name("package")
        .takes_value(true)
        .help("Filer packages category/name using a regexp");
    let args = app_from_crate!()
        .setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(Arg::with_name("logfile")
             .long("logfile")
             .short("f")
             .takes_value(true)
             .default_value("/var/log/emerge.log")
             .help("Location of emerge log file"))
        .subcommand(SubCommand::with_name("list")
                    .about("Show full merge history")
                    .arg(&arg_pkg))
        .subcommand(SubCommand::with_name("stats")
                    .about("Show merge stats")
                    .arg(&arg_pkg)
                    .arg(&arg_limit))
        .subcommand(SubCommand::with_name("predict")
                    .about("Predict merge time for packages listed by 'emerge -p'")
                    .arg(&arg_limit))
        .get_matches();

    let mut tw = TabWriter::new(io::stdout());
    match args.subcommand() {
        ("list",    Some(sub_args)) => cmd_list(&args, sub_args),
        ("stats",   Some(sub_args)) => cmd_stats(&mut tw, &args, sub_args),
        ("predict", Some(sub_args)) => cmd_predict(&mut tw, &args, sub_args),
        (other, _) => unimplemented!("{} subcommand", other),
    }.unwrap();
    tw.flush().unwrap();
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
