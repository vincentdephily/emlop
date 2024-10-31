#![cfg_attr(feature = "unstable", feature(test))]

mod commands;
mod config;
mod datetime;
mod parse;
mod table;

use crate::{config::*, datetime::*};
use anyhow::Error;
use log::*;
use std::str::FromStr;

fn main() {
    let res = match Configs::load() {
        Ok(Configs::Log(gc, sc)) => commands::cmd_log(gc, sc),
        Ok(Configs::Stats(gc, sc)) => commands::cmd_stats(gc, sc),
        Ok(Configs::Predict(gc, sc)) => commands::cmd_predict(gc, sc),
        Ok(Configs::Accuracy(gc, sc)) => commands::cmd_accuracy(gc, sc),
        Ok(Configs::Complete(gc, sc)) => commands::cmd_complete(gc, sc),
        Err(e) => Err(e),
    };
    match res {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(e) => {
            match e.downcast::<clap::Error>() {
                Ok(ce) => ce.format(&mut build_cli()).print().unwrap_or(()),
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
