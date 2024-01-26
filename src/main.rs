#![cfg_attr(feature = "unstable", feature(test))]

mod cli;
mod commands;
mod config;
mod datetime;
mod parse;
mod proces;
mod table;

use crate::{commands::*, config::*, datetime::*, parse::AnsiStr};
use anyhow::Error;
use clap::{error::ErrorKind, ArgMatches, Error as ClapErr};
use log::*;
use std::{io::IsTerminal, str::FromStr};

fn main() {
    let res = match Configs::load() {
        Ok(Configs::Log(args, gc, sc)) => cmd_log(&args, &gc, &sc),
        Ok(Configs::Stats(args, gc, sc)) => cmd_stats(&args, &gc, &sc),
        Ok(Configs::Predict(args, gc, sc)) => cmd_predict(&args, &gc, &sc),
        Ok(Configs::Accuracy(args, gc, sc)) => cmd_accuracy(&args, &gc, &sc),
        Ok(Configs::Complete(args)) => cmd_complete(&args),
        Err(e) => Err(e),
    };
    match res {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(e) => match e.downcast::<ClapErr>() {
            Ok(ce) => ce.format(&mut cli::build_cli()).exit(),
            Err(e) => {
                log_err(e);
                std::process::exit(2)
            },
        },
    }
}

pub fn log_err(e: Error) {
    match e.source() {
        Some(s) => error!("{}: {}", e, s),
        None => error!("{}", e),
    }
}

/// Parse and return optional argument from an ArgMatches
///
/// This is similar to clap's `get_one()` with `value_parser` except it allows late parsing with an
/// argument.
pub fn get_parse<T, P, A>(args: &ArgMatches,
                          name: &str,
                          parse: P,
                          arg: A)
                          -> Result<Option<T>, ClapErr>
    where P: FnOnce(&str, A) -> Result<T, &'static str>
{
    match args.get_one::<String>(name) {
        None => Ok(None),
        Some(s) => match parse(s, arg) {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(ClapErr::raw(ErrorKind::InvalidValue,
                                       format!("\"{s}\" isn't a valid for '--{name}': {e}"))),
        },
    }
}

/// Alias to `write!(...).expect("write to buf")` just to save on typing/indent.
#[macro_export]
macro_rules! wtb { ($b:ident, $($arg:expr),+) => {write!($b, $($arg),+).expect("write to buf")} }


#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ResumeKind {
    #[clap(alias("a"))]
    Any,
    #[clap(alias("m"))]
    Main,
    #[clap(alias("b"))]
    Backup,
    #[clap(alias("n"))]
    No,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum DurationStyle {
    HMS,
    #[clap(id("hmsfixed"))]
    HMSFixed,
    #[clap(alias("s"))]
    Secs,
    #[clap(alias("h"))]
    Human,
}

#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ColorStyle {
    #[clap(alias("y"))]
    Always,
    #[clap(alias("n"))]
    Never,
}

#[derive(Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum OutStyle {
    #[clap(alias("c"))]
    Columns,
    #[clap(alias("t"))]
    Tab,
}
