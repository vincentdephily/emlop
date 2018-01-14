//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! I initially tried using the procinfo and/or nix crates, but gettinng a usable process start time
//! with them seemed unacceptably complicated. I also didn't want to rely on external tools like ps,
//! as this is slow and difficult to make portable. Right now I have something that's fast (?) and
//! simple, but might not work outside of Linux and could do with more flexibility and better error
//! checking.

use std::fs;
use std::io;
use std::io::prelude::*;
use std::str::FromStr;

use ::*;

#[derive(Debug)]
pub struct Info {
    comm: String,
    cmdline: String,
    start: i64,
    pid: u32,
}

fn get_proc_info(entry: fs::DirEntry) -> Result<Info, io::Error> {
    // Parse command name
    let mut comm = String::new();
    fs::File::open(entry.path().join("comm"))?.read_to_string(&mut comm)?;
    comm = comm.trim().into();
    // Parse arguments
    let mut cmdline = String::new();
    fs::File::open(entry.path().join("cmdline"))?.read_to_string(&mut cmdline)?;
    cmdline = cmdline.replace("\0", " ").trim().into();
    // Parse start time
    let start = epoch(entry.metadata()?.modified()?);
    // Parse pid
    let pid = u32::from_str(&entry.file_name().to_string_lossy()).unwrap_or(0);
    // Done
    Ok(Info{comm: comm,
            cmdline: cmdline,
            start: start,
            pid: pid,
    })
}

fn get_all_info() -> Result<Vec<Info>, io::Error> {
    let mut ret: Vec<Info> = Vec::new();
    for entry in fs::read_dir("/proc/")? {
        let entry = entry?;
        if let Ok(i) = get_proc_info(entry) {
            ret.push(i)
        }
    }
    Ok(ret)
}

/// Print a message for each running emerge process and return the start time of the oldest emerge process.
pub fn current_merge_start() -> i64 {
    let info = get_all_info().unwrap();
    let now = epoch(SystemTime::now());
    let mut first_merge = std::i64::MAX;
    for i in info {
        if i.comm == "emerge" { //FIXME: test this earlyer
            first_merge = std::cmp::min(first_merge, i.start);
            println!("emerge ... {} (pid {})   {}", &i.cmdline[(i.cmdline.len()-20)..], i.pid, fmt_duration(now-i.start));
        }
    }
    first_merge
}
