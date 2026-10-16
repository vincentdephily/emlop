#![cfg_attr(feature = "unstable", feature(test))]

mod commands;
mod config;
mod datetime;
mod parse;
mod table;

use crate::config::Configs;
use emlop_lib::ArgError;
use log::error;

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
                Ok(ce) => ce.print().unwrap_or(()),
                Err(e) => match e.downcast::<ArgError>() {
                    Ok(ae) => eprintln!("{}", ClapArgErr(ae)),
                    Err(e) => match e.source() {
                        Some(s) => error!("{e}: {s}"),
                        None => error!("{e}"),
                    },
                },
            }
            std::process::exit(2)
        },
    }
}

/// Wrapper around ArgError that mimic theformating of clap errors
struct ClapArgErr(ArgError);
impl std::fmt::Display for ClapArgErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let (r, g, b, c) = ("\x1B[1;31m", "\x1B[32m", "\x1B[33m", "\x1B[m");
        let (v, s) = (&self.0.val, self.0.src);
        match &self.0.msg {
            Some(m) => write!(f, "{r}error{c}: invalid value '{b}{v}{c}' for '{g}{s}{c}': {m}")?,
            None => write!(f, "{r}error{c}: invalid value '{b}{v}{c}' for '{g}{s}{c}'")?,
        }
        if let Some(pos) = &self.0.pos {
            if pos.contains(' ') {
                let mut sep = "\n  possible values: ";
                for p in pos.split_ascii_whitespace() {
                    write!(f, "{sep}{g}{p}{c}")?;
                    sep = ", ";
                }
            } else {
                let mut sep = "\n  possible value: combination of ";
                for p in pos.chars() {
                    write!(f, "{sep}{g}{p}{c}")?;
                    sep = ", ";
                }
            }
        }
        write!(f, "\n\nFor more information, try '{g}--help{c}'.")
    }
}

/// Alias to `write!(...).expect("write to buf")` just to save on typing/indent.
#[macro_export]
macro_rules! wtb { ($b:ident, $($arg:expr),+) => {write!($b, $($arg),+).expect("write to buf")} }
