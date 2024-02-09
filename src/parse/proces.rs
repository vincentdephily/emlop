//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use crate::*;
use anyhow::{ensure, Context};
use std::{fs::{read_dir, DirEntry, File},
          io::prelude::*,
          path::PathBuf};

#[derive(Debug)]
pub struct Proc {
    pub idx: usize,
    pub cmdline: String,
    pub start: i64,
    pub pid: i32,
}

impl std::fmt::Display for Proc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pid = format!("Pid {}: ", self.pid);
        let capacity = f.precision().unwrap_or(45).saturating_sub(pid.len());
        let cmdlen = self.cmdline.len();
        if capacity >= cmdlen || cmdlen < 4 {
            write!(f, "{pid}{}", &self.cmdline)
        } else if capacity > 3 {
            write!(f, "{pid}...{}", &self.cmdline[(cmdlen - capacity + 3)..])
        } else {
            write!(f, "{pid}...")
        }
    }
}

/// Get command name, arguments, start time, and pid for one process.
fn get_proc_info(filter: &[&str],
                 entry: &DirEntry,
                 clocktick: i64,
                 time_ref: i64,
                 tmpdirs: &mut Vec<PathBuf>)
                 -> Option<Proc> {
    // Parse pid.
    // At this stage we expect `entry` to not always correspond to a process.
    let pid = i32::from_str(&entry.file_name().to_string_lossy()).ok()?;
    // See linux/Documentation/filesystems/proc.txt Table 1-4: Contents of the stat files.
    let mut stat = String::new();
    File::open(entry.path().join("stat")).ok()?.read_to_string(&mut stat).ok()?;
    // Parse command name, bail out now if it doesn't match.
    // The command name is surrounded by parens and may contain spaces.
    let (cmd_start, cmd_end) = (stat.find('(')? + 1, stat.rfind(')')?);
    let idx = if filter.is_empty() {
        usize::MAX
    } else {
        filter.iter().position(|&f| stat[cmd_start..cmd_end].starts_with(f))?
    };
    // Parse start time
    let start_time = i64::from_str(stat[cmd_end + 1..].split(' ').nth(20)?).ok()?;
    // Parse arguments
    let mut cmdline = String::new();
    File::open(entry.path().join("cmdline")).ok()?.read_to_string(&mut cmdline).ok()?;
    cmdline = cmdline.replace('\0', " ").trim().into();
    // Find portage tmpdir
    extend_tmpdirs(entry.path(), tmpdirs);
    // Done
    Some(Proc { idx, cmdline, start: time_ref + start_time / clocktick, pid })
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

/// Get command name, arguments, start time, and pid for all processes.
pub fn get_all_info(filter: &[&str], tmpdirs: &mut Vec<PathBuf>) -> Vec<Proc> {
    get_all_info_result(filter, tmpdirs).unwrap_or_else(|e| {
                                            log_err(e);
                                            vec![]
                                        })
}
fn get_all_info_result(filter: &[&str], tmpdirs: &mut Vec<PathBuf>) -> Result<Vec<Proc>, Error> {
    // clocktick and time_ref are needed to interpret stat.start_time. time_ref should correspond to
    // the system boot time; not sure why it doesn't, but it's still usable as a reference.
    // SAFETY: returns a system constant, only failure mode should be a zero/negative value
    let clocktick: i64 = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    ensure!(clocktick > 0, "Failed getting system clock ticks");
    let mut uptimestr = String::new();
    File::open("/proc/uptime").context("Opening /proc/uptime")?
                              .read_to_string(&mut uptimestr)
                              .context("Reading /proc/uptime")?;
    let uptime = i64::from_str(uptimestr.split('.').next().unwrap()).unwrap();
    let time_ref = epoch_now() - uptime;
    // Now iterate through /proc/<pid>
    let mut ret: Vec<Proc> = Vec::new();
    for entry in read_dir("/proc/").context("Listing /proc/")? {
        if let Some(i) = get_proc_info(filter, &entry?, clocktick, time_ref, tmpdirs) {
            ret.push(i)
        }
    }
    Ok(ret)
}


#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::{collections::BTreeMap, process::Command};
    use time::{macros::format_description, PrimitiveDateTime};

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
        let mut info = get_all_info(&[], &mut tmpdirs)
            .iter()
            .map(|i| (i.pid, (i.cmdline.clone(), Some(i.start), None)))
            .collect::<BTreeMap<i32, (String, Option<i64>, Option<i64>)>>();
        // Then get them using the ps implementation (merging them into the same data structure)
        let ps_start = epoch_now();
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
                (c, Some(t), None) =>                           {println!("WARN {pid} {} disappeared after rust run\t{c}", fmt_utctime(t)); 1},
                (c, None, Some(t)) if t >= ps_start -1 =>       {println!("WARN {pid} {} appeared right after rust run\t{c}", fmt_utctime(t)); 1},
                (c, None, Some(t)) =>                           {println!("ERR  {pid} {} seen by ps but not by rust\t{c}", fmt_utctime(t)); 10},
                (c, Some(tr), Some(tp)) if (tr-tp).abs() < 2 => {println!("OK   {pid} {} {} secs diff\t{c}", fmt_utctime(tr), tr-tp); 0},
                (c, Some(tr), Some(tp)) if (tr-tp).abs() < 5 => {println!("WARN {pid} {} {} secs diff\t{c}", fmt_utctime(tr), tr-tp); 1},
                (c, Some(tr), Some(tp)) =>                      {println!("ERR  {pid} {} {} secs diff\t{c}", fmt_utctime(tr), tr-tp); 5},
                (c, None, None) =>                              {println!("ERR  {pid}: no times\t{c}"); 10},
            }
        }
        assert!(e < 10, "Got failure score of {e}");
    }

    #[test]
    fn format_info() {
        let s = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let t: Vec<(i32, usize, usize, &str)> = vec![// Precison is way too small, use elipsis starting at 4 chars
                                                     (1, 1, 1, "Pid 1: a"),
                                                     (1, 2, 1, "Pid 1: ab"),
                                                     (2, 3, 1, "Pid 2: abc"),
                                                     (3, 4, 1, "Pid 3: ..."),
                                                     (4, 5, 1, "Pid 4: ..."),
                                                     (330, 1, 1, "Pid 330: a"),
                                                     (331, 2, 1, "Pid 331: ab"),
                                                     (332, 3, 1, "Pid 332: abc"),
                                                     (333, 4, 1, "Pid 333: ..."),
                                                     (334, 5, 1, "Pid 334: ..."),
                                                     // Here we have enough space
                                                     (1, 1, 12, "Pid 1: a"),
                                                     (1, 2, 12, "Pid 1: ab"),
                                                     (1, 3, 12, "Pid 1: abc"),
                                                     (1, 4, 12, "Pid 1: abcd"),
                                                     (1, 5, 12, "Pid 1: abcde"),
                                                     (12, 4, 12, "Pid 12: abcd"),
                                                     (123, 3, 12, "Pid 123: abc"),
                                                     (1234, 2, 12, "Pid 1234: ab"),
                                                     // Running out of space again, but we can display part of it
                                                     (1, 6, 12, "Pid 1: ...ef"),
                                                     (1, 7, 12, "Pid 1: ...fg"),
                                                     (1, 8, 12, "Pid 1: ...gh"),
                                                     (22, 9, 12, "Pid 22: ...i"),];
        for (pid, cmdlen, precision, out) in t.into_iter() {
            dbg!((pid, cmdlen, precision, out));
            let i = Proc { idx: usize::MAX, pid, cmdline: s[..cmdlen].to_string(), start: 0 };
            let f = format!("{1:.0$}", precision, i);
            assert!(precision < 10 || f.len() <= precision, "{} <= {}", f.len(), precision);
            assert_eq!(f, out);
        }
    }
}
