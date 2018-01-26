#[macro_use]
extern crate clap;
extern crate chrono;
extern crate regex;
extern crate sysconf;

mod commands;
mod parser;
mod proces;

use clap::{AppSettings, Arg, SubCommand};
use chrono::{DateTime, Local, TimeZone};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use commands::*;

fn main() {
    let arg_limit = Arg::with_name("limit")
        .long("limit")
        .short("l")
        .takes_value(true)
        .default_value("10")
        .validator(is_posint)
        .help("Use the last N merge time to predict future merge time");
    let arg_pkg = Arg::with_name("package")
        .takes_value(true)
        .help("Regexp to match package name");
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
        .subcommand(SubCommand::with_name("summary")
                    .about("Show merge stats summary")
                    .arg(&arg_pkg)
                    .arg(&arg_limit))
        .subcommand(SubCommand::with_name("predict")
                    .about("Predict merge time for packages listed by 'emerge -p'")
                    .arg(&arg_limit))
        .get_matches();

    match args.subcommand() {
        ("list",    Some(sub_args)) => cmd_list(args.value_of("logfile").unwrap(), sub_args),
        ("summary", Some(sub_args)) => cmd_summary(args.value_of("logfile").unwrap(), sub_args),
        ("predict", Some(sub_args)) => cmd_predict(args.value_of("logfile").unwrap(), sub_args),
        (other, _) => unimplemented!("{} subcommand", other),
    }.unwrap();
}

fn is_posint(v: String) -> Result<(), String> {
    match u32::from_str(&v) {
        Ok(id) if id > 0 => Ok(()),
        _ => Err("Must be an positive integer.".into()),
    }
}

pub fn fmt_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = secs % 3600 / 60;
    let s = secs % 60;
    if h > 0      { format!("{:02}:{:02}:{:02}", h, m, s) }
    else if m > 0 { format!(      "{:02}:{:02}", m, s) }
    else          { format!(            "{:02}", s) }
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
