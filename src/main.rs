#[macro_use]
extern crate clap;
extern crate chrono;
extern crate regex;

mod parser;

use chrono::{Local, TimeZone};
use clap::{AppSettings, Arg, ArgMatches, SubCommand};
use std::collections::HashMap;
use std::io;

use parser::Event;

fn main() {
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
                    .about("List full merge history")
                    .arg(Arg::with_name("package")
                         .takes_value(true)
                         .help("Regexp to match package name"))

        )
        .get_matches();

    match args.subcommand() {
        ("list", Some(sub_args)) => cmd_list(args.value_of("logfile").unwrap(), sub_args),
        (other, _) => unimplemented!("{} subcommand", other),
    };
}

fn pretty_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = secs % 3600 / 60;
    let s = secs % 60;
    if h > 0      { format!("{:02}:{:02}:{:02}", h, m, s) }
    else if m > 0 { format!(      "{:02}:{:02}", m, s) }
    else          { format!(            "{:02}", s) }
}
}

fn cmd_list(filename: &str, args: &ArgMatches) -> Result<(), io::Error> {
    let parser = parser::Parser::new(filename, args.value_of("package"));
    let mut started: HashMap<(String,String,String), i64> = HashMap::new();
    for event in parser {
        match event {
            Event::Start{ts, ebuild, version, iter} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            Event::Stop{ts, ebuild, version, iter} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(prevts) => println!("{} {:>9} {}-{}",     Local.timestamp(ts, 0), pretty_duration(ts - prevts), ebuild, version),
                    None =>         println!("{}  00:00:00 {}-{}", Local.timestamp(ts, 0), ebuild, version),
                }
            },
        }
    };
    Ok(())
}
