#[macro_use]
extern crate clap;
extern crate chrono;
extern crate regex;

mod parser;

use chrono::{Local, TimeZone};
use clap::{AppSettings, Arg, ArgMatches, SubCommand};
use std::collections::HashMap;
use std::io;
use std::str::FromStr;

use parser::*;

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

pub fn fmt_time(ts: i64) -> chrono::DateTime<Local> {
    Local.timestamp(ts, 0)
}

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
fn cmd_list(filename: &str, args: &ArgMatches) -> Result<(), io::Error> {
    let hist = HistParser::new(filename, args.value_of("package"));
    let mut started: HashMap<(String,String,String), i64> = HashMap::new();
    for event in hist {
        match event {
            HistEvent::Start{ts, ebuild, version, iter} => {
                // This'll overwrite any previous entry, if a build started but never finished
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            HistEvent::Stop{ts, ebuild, version, iter} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(prevts) => println!("{} {:>9} {}-{}",     fmt_time(ts), fmt_duration(ts - prevts), ebuild, version),
                    None =>         println!("{}  00:00:00 {}-{}", fmt_time(ts), ebuild, version),
                }
            },
        }
    };
    Ok(())
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
fn cmd_summary(filename: &str, args: &ArgMatches) -> Result<(), io::Error> {
    let parser = HistParser::new(filename, args.value_of("package"));
    let lim = value_t!(args, "limit", usize).unwrap();
    let mut started: HashMap<(String,String,String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    let mut maxlen = 0;
    for event in parser {
        match event {
            HistEvent::Start{ts, ebuild, version, iter} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            HistEvent::Stop{ts, ebuild, version, iter} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(start_ts) => {
                        maxlen = maxlen.max(ebuild.len());
                        let timevec = times.entry(ebuild.clone()).or_insert(vec![]);
                        timevec.insert(0, ts-start_ts);
                    },
                    None => (),
                }
            }
        }
    };
    for (pkg,tv) in times.iter() {
        let (predtime,predcount,tottime,totcount) = tv.iter()
            .fold((0,0,0,0), |(pt,pc,tt,tc), &i| {
                if tc >= lim {(pt,  pc,  tt+i,tc+1)}
                else         {(pt+i,pc+1,tt+i,tc+1)}
            });
        println!("{:width$} {:>9}/{:<4} {:>8}", pkg, fmt_duration(tottime), totcount, fmt_duration(predtime/predcount), width=maxlen);
    }
    Ok(())
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds parsed from stdin.
fn cmd_predict(filename: &str, args: &ArgMatches) -> Result<(), io::Error> {
    let hist = HistParser::new(filename, None);
    let pretend = PretendParser::new();
    let lim = value_t!(args, "limit", usize).unwrap();
    let mut started: HashMap<(String,String,String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    let mut maxlen = 0;
    for event in hist {
        match event {
            HistEvent::Start{ts, ebuild, version, iter} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            HistEvent::Stop{ts, ebuild, version, iter} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(start_ts) => {
                        maxlen = maxlen.max(ebuild.len());
                        let timevec = times.entry(ebuild.clone()).or_insert(vec![]);
                        timevec.insert(0, ts-start_ts);
                    },
                    None => (),
                }
            }
        }
    }
    let mut tottime = 0;
    let mut totcount = 0;
    for PretendEvent{ebuild, version} in pretend {
        if let Some(tv) = times.get(&ebuild) {
            let (predtime,predcount,_) = tv.iter()
                .fold((0,0,0), |(pt,pc,tc), &i| {
                    if tc >= lim {(pt,  pc,  tc+1)}
                    else         {(pt+i,pc+1,tc+1)}
                });
            tottime += predtime/predcount;
            totcount += 1;
            println!("{:width$} {:>9}", ebuild, pretty_duration(predtime/predcount), width=maxlen);
        } else {
            println!("{:width$}", ebuild, width=maxlen);
        }
    }
    println!("{:width$} {:>9}/{}", "Total:", pretty_duration(tottime), totcount, width=maxlen);
    Ok(())
}
