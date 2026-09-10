//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use anyhow::{Context, Error, ensure};
use atoi::atoi;
use libc::pid_t;
use log::{debug, error};
use std::{collections::BTreeMap,
          fs::{DirEntry, File, read_dir, read_to_string},
          io::prelude::*,
          path::PathBuf,
          str::FromStr};
use time::Timestamp;

#[derive(Debug, Clone, Copy)]
pub enum ProcKind {
    Emerge,
    Sandbox,
    Other,
}

#[derive(Debug)]
pub struct Proc {
    pub kind: ProcKind,
    pub cmdline: String,
    pub start: i64,
    pub pid: pid_t,
    pub ppid: pid_t,
}

pub type ProcList = BTreeMap<pid_t, Proc>;

/// Gather info for all processes
pub fn get_all_proc(tmpdirs: &mut Vec<PathBuf>) -> ProcList {
    get_all_proc_result(tmpdirs).unwrap_or_else(|e| {
                                    match e.source() {
                                        Some(s) => error!("{e}: {s}"),
                                        None => error!("{e}"),
                                    };
                                    //                          log_err(e);
                                    BTreeMap::new()
                                })
}
fn get_all_proc_result(tmpdirs: &mut Vec<PathBuf>) -> Result<ProcList, Error> {
    // clocktick and time_ref are needed to interpret stat.start_time.
    // SAFETY: returns a system constant, only failure mode should be a zero/negative value
    let clocktick: i64 = unsafe {
        #[allow(clippy::useless_conversion)] // `sysconf()` returns `i32` on 32bit platforms
        libc::sysconf(libc::_SC_CLK_TCK).into()
    };
    ensure!(clocktick > 0, "Failed getting system clock ticks");
    let mut uptimebuf = Vec::with_capacity(32);
    File::open("/proc/uptime").context("Opening /proc/uptime")?
                              .read_to_end(&mut uptimebuf)
                              .context("Reading /proc/uptime")?;
    let uptime = atoi::<i64>(&uptimebuf).context("Parsing /proc/uptime")?;
    let time_ref = Timestamp::now().as_seconds() - uptime;
    // Now iterate through /proc/<pid>
    let mut ret: BTreeMap<pid_t, Proc> = BTreeMap::new();
    for entry in read_dir("/proc/").context("Listing /proc/")?.filter_map(Result::ok) {
        if let Some(p) = get_proc(&entry, clocktick, time_ref, tmpdirs) {
            ret.insert(p.pid, p);
        }
    }
    Ok(ret)
}

/// Fill `Proc` struct for one process, and update tmpdirs
fn get_proc(entry: &DirEntry,
            clocktick: i64,
            time_ref: i64,
            tmpdirs: &mut Vec<PathBuf>)
            -> Option<Proc> {
    // Parse pid.
    // At this stage we expect `entry` to not always correspond to a process.
    let pid = i32::from_str(&entry.file_name().to_string_lossy()).ok()?;
    // See linux/Documentation/filesystems/proc.rst Table 1-4: Contents of the stat files.
    let stat = read_to_string(entry.path().join("stat")).ok()?;
    // Parse command name (it's surrounded by parens and may contain spaces)
    // If it's emerge, look for portage tmpdir in its fds
    let (cmd_start, cmd_end) = (stat.find('(')? + 1, stat.rfind(')')?);
    let kind = if &stat[cmd_start..cmd_end] == "emerge" {
        extend_tmpdirs(entry.path(), tmpdirs);
        ProcKind::Emerge
    } else if stat[cmd_start..cmd_end].starts_with("python")
              || stat[cmd_start..cmd_end] == *"sandbox"
    {
        ProcKind::Sandbox
    } else {
        ProcKind::Other
    };
    // Parse parent pid and start time
    let mut fields = stat[cmd_end + 1..].split(' ');
    let ppid = i32::from_str(fields.nth(2)?).ok()?;
    let start_time = i64::from_str(fields.nth(17)?).ok()?;
    // Parse arguments
    let cmdline = read_to_string(entry.path().join("cmdline")).ok()?;
    // Done
    Some(Proc { kind, cmdline, start: time_ref + start_time / clocktick, pid, ppid })
}

/// Find tmpdir by looking for "build.log" in the process fds, and add it to the provided vector.
fn extend_tmpdirs(proc: PathBuf, tmpdirs: &mut Vec<PathBuf>) {
    if let Ok(entries) = read_dir(proc.join("fd")) {
        let procstr = proc.to_string_lossy();
        for d in entries.filter_map(|e| {
                            let p = e.ok()?.path().canonicalize().ok()?;
                            if p.file_name() != Some(std::ffi::OsStr::new("build.log")) {
                                return None;
                            }
                            let d = p.parent()?.parent()?.parent()?.parent()?.parent()?;
                            debug!("Tmpdir {} found in {}", d.to_string_lossy(), procstr);
                            Some(d.to_path_buf())
                        })
        {
            if !tmpdirs.contains(&d) {
                // Insert at the front because it's a better candidate than cli/default tmpdir
                tmpdirs.insert(0, d)
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::FmtUtc;
    use regex::Regex;
    use std::{collections::BTreeMap, process::Command};
    use time::{PrimitiveDateTime, macros::format_description};

    fn parse_ps_time(s: &str) -> i64 {
        let fmt = format_description!("[month repr:short] [day padding:space] [hour]:[minute]:[second] [year]");
        PrimitiveDateTime::parse(s, &fmt).expect(&format!("Cannot parse {}", s))
                                         .assume_utc() // We run ps with TZ=UTC
                                         .unix_timestamp()
    }

    /// Check that our impl get similar results as `ps`
    ///
    /// Ignored by default: False negatives on very busy systems
    #[test]
    #[ignore]
    #[rustfmt::skip]
    fn start_time() {
        // First get the system's process start times using our implementation
        // Store it as pid => (cmd, rust_time, ps_time)
        let mut tmpdirs = vec![];
        let mut info = get_all_proc(&mut tmpdirs)
            .iter()
            .map(|(pid, i)| (*pid, (i.cmdline.clone(), Some(i.start), None)))
            .collect::<BTreeMap<pid_t, (String, Option<i64>, Option<i64>)>>();
        // Then get them using the ps implementation (merging them into the same data structure)
        let ps_start = Timestamp::now().as_seconds();
        let cmd = Command::new("ps").env("TZ", "UTC")
                                    .env("LC_ALL", "C") // Use a consistent format for datetimes
                                    .args(&["-o",
                                            "pid,lstart", // Output pid and start time
                                            "-ax", // All processes including those "not associated with a terminal"
                                            "--no-header"]) // No headers
                                    .output()
                                    .expect("failed to execute ps");
        let re = Regex::new("^ *([0-9]+) [A-Za-z]+ ([a-zA-Z0-9: ]+)$").unwrap();
        for lineres in cmd.stdout.lines() {
            if let Ok(line) = lineres {
                match re.captures(&line) {
                    Some(c) => {
                        let pid = c.get(1).unwrap().as_str().parse::<i32>().unwrap();
                        let time = parse_ps_time(c.get(2).unwrap().as_str());
                        info.entry(pid)
                            .and_modify(|t| t.2 = Some(time))
                            .or_insert(("?".into(), None, Some(time)));
                    },
                    None => assert!(false, "Couldn't parse {}", line),
                }
            }
        }
        // Check the results. For debugging purposes it's usefull to print everything and only
        // assert at the end. Also some cases are considered soft errors and only make the whole
        // test fail if they happen a lot.
        assert!(info.len() > 5, "Only {} processes found", info.len());
        let mut e: u32 = 0;
        for (pid, times) in info {
            e += match times {
                (c, Some(t), None) =>                           {println!("WARN {pid} {} disappeared after rust run\t{c}", FmtUtc(t)); 1},
                (c, None, Some(t)) if t >= ps_start -1 =>       {println!("WARN {pid} {} appeared right after rust run\t{c}", FmtUtc(t)); 1},
                (c, None, Some(t)) =>                           {println!("ERR  {pid} {} seen by ps but not by rust\t{c}", FmtUtc(t)); 10},
                (c, Some(tr), Some(tp)) if (tr-tp).abs() < 2 => {println!("OK   {pid} {} {} secs diff\t{c}", FmtUtc(tr), tr-tp); 0},
                (c, Some(tr), Some(tp)) if (tr-tp).abs() < 5 => {println!("WARN {pid} {} {} secs diff\t{c}", FmtUtc(tr), tr-tp); 1},
                (c, Some(tr), Some(tp)) =>                      {println!("ERR  {pid} {} {} secs diff\t{c}", FmtUtc(tr), tr-tp); 5},
                (c, None, None) =>                              {println!("ERR  {pid}: no times\t{c}"); 10},
            }
        }
        assert!(e < 10, "Got failure score of {e}");
    }
}

#[cfg(feature = "unstable")]
#[cfg(test)]
mod bench {
    use super::*;
    extern crate test;

    #[bench]
    /// Bench listing all processes
    fn get_all(b: &mut test::Bencher) {
        b.iter(move || {
             let mut tmpdirs = vec![];
             get_all_proc(&mut tmpdirs);
         });
    }
}
