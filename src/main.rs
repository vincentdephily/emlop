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
use log::*;
use std::{io::IsTerminal, str::FromStr};

fn main() {
    let res = match Configs::load() {
        Ok(Configs::Log(gc, sc)) => cmd_log(&gc, &sc),
        Ok(Configs::Stats(args, gc, sc)) => cmd_stats(&args, &gc, &sc),
        Ok(Configs::Predict(args, gc, sc)) => cmd_predict(&args, &gc, &sc),
        Ok(Configs::Accuracy(args, gc, sc)) => cmd_accuracy(&args, &gc, &sc),
        Ok(Configs::Complete(args)) => cmd_complete(&args),
        Err(e) => Err(e),
    };
    match res {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(e) => {
            match e.downcast::<clap::Error>() {
                Ok(ce) => ce.format(&mut cli::build_cli()).print().unwrap_or(()),
                Err(e) => match e.downcast::<ArgError>() {
                    Ok(ae) => eprintln!("{ae}"),
                    Err(e) => log_err(e),
                },
            }
            std::process::exit(2)
        },
    }
}

pub fn log_err(e: Error) {
    match e.source() {
        Some(s) => error!("{}: {}", e, s),
        None => error!("{}", e),
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
