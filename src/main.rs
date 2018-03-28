#[cfg(test)]
extern crate assert_cli;
extern crate atty;
extern crate chrono;
extern crate clap;
#[cfg(test)]
#[macro_use]
extern crate indoc;
extern crate regex;
#[macro_use]
extern crate structopt;
extern crate sysconf;
extern crate tabwriter;

mod commands;
mod parser;
mod proces;

use chrono::{DateTime, Local, TimeZone};
use clap::AppSettings;
use std::io;
use std::io::Write;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use structopt::StructOpt;
use tabwriter::TabWriter;

use commands::*;

#[derive(StructOpt)]
#[structopt(raw(global_settings="&[AppSettings::InferSubcommands, AppSettings::DeriveDisplayOrder, AppSettings::VersionlessSubcommands, AppSettings::DisableHelpSubcommand]"))]
pub struct Opt {
    #[structopt(help="Location of emerge log file.", short="f", long="logfile", default_value="/var/log/emerge.log", raw(global="true"))]
    logfile: String,
    #[structopt(help="Only consider events from that date onward.", long="from", raw(global="true", validator="is_posint"))]
    mindate: Option<i64>,
    #[structopt(help="Only consider events up to that date.", long="to", raw(global="true", validator="is_posint"))]
    maxdate: Option<i64>,
    #[structopt(subcommand)]
    subcmd: SubCmd,
}

#[derive(StructOpt)]
enum SubCmd {
    #[structopt(name="list", about="Show merge history.",
                long_about="Show merge history.\nShows date, merge time, package name-version of completed merges.")]
    List(ListOpt),
    #[structopt(name="predict", about="Predict merge time for current/pretended merges.",
                long_about="Predict merge time.\n\
                            If input is a terminal, predict time for the current merge (if any).\n\
                            If input is an emerge pretend output (for example run `emerge -rOp|emlop p`), predict time for those merges.")]
    Predict(PredictOpt),
    #[structopt(name="stats", about="Show merge stats.",
                long_about="Show merge stats.\n\nStatistics include total merge time, total merge count, and merge time prediction.")]
    Stats(StatsOpt),
}

#[derive(StructOpt)]
pub struct ListOpt {
    #[structopt(help="Display only packages matching <package>.")]
    package: Option<String>,
    #[structopt(help="Interpret <package> as a string instead of a regexp.",
                long_help="Match packages using exact string instead of regexp. \
                           Without this flag, matching is done by case-insensitive regexp (see https://docs.rs/regex/0.2.8/regex/index.html#syntax) on 'category/name'. \
                           With this flag, matching is done by case-sentitive string on 'name' (or 'category/name' if <package> contains a /).", //FIXME crate version
                short="e")]
    exact: bool,
}

#[derive(StructOpt)]
pub struct PredictOpt {
    #[structopt(help="Use the last N merge times to predict future merge time.",
                short="l", long="limit", default_value="10", raw(validator="is_posint"))]
    limit: u32,
}

#[derive(StructOpt)]
pub struct StatsOpt {
    #[structopt(help="Display only packages matching <package>.")]
    package: Option<String>,
    #[structopt(help="Interpret <package> as a string instead of a regexp.",
                long_help="Match packages using exact string instead of regexp. \
                           Without this flag, matching is done by case-insensitive regexp (see https://docs.rs/regex/0.2.8/regex/index.html#syntax) on 'category/name'. \
                           With this flag, matching is done by case-sentitive string on 'name' (or 'category/name' if <package> contains a /).", //FIXME crate version
                short="e")]
    exact: bool,
    #[structopt(help="Use the last N merge times to predict future merge time.",
                short="l", long="limit", default_value="10", raw(validator="is_posint"))]
    limit: u32,
}

fn main() {
    let opt = Opt::from_args();
    let mut tw = TabWriter::new(io::stdout());
    match &opt.subcmd {
        &SubCmd::List{0:ref s} => cmd_list(&opt, s),
        &SubCmd::Stats{0:ref s} => cmd_stats(&mut tw, &opt, s),
        &SubCmd::Predict{0:ref s} => cmd_predict(&mut tw, &opt, s),
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
