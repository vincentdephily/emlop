//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use crate::*;
use std::{fs::{read_dir, DirEntry, File},
          io::{self, prelude::*}};
use sysconf::raw::{sysconf, SysconfVariable};

#[derive(Debug)]
pub struct Info {
    pub cmdline: String,
    pub start: i64,
    pub pid: i32,
}

impl std::fmt::Display for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pid = format!("Pid {}: ", self.pid);
        let capacity = f.precision().unwrap_or(100).saturating_sub(pid.len());
        let cmdlen = self.cmdline.len();
        if capacity >= cmdlen || cmdlen < 4 {
            write!(f, "{}{}", pid, &self.cmdline)
        } else if capacity > 3 {
            write!(f, "{}...{}", pid, &self.cmdline[(cmdlen - capacity + 3)..])
        } else {
            write!(f, "{}...", pid)
        }
    }
}


/// Get command name, arguments, start time, and pid for one process.
fn get_proc_info(filter: Option<&str>,
                 entry: &DirEntry,
                 clocktick: i64,
                 time_ref: i64)
                 -> Option<Info> {
    // Parse pid.
    // At this stage we expect `entry` to not always correspond to a process.
    let pid = i32::from_str(&entry.file_name().to_string_lossy()).ok()?;
    // See linux/Documentation/filesystems/proc.txt Table 1-4: Contents of the stat files.
    let mut stat = String::new();
    File::open(entry.path().join("stat")).ok()?.read_to_string(&mut stat).ok()?;
    // Parse command name, bail out now if it doesn't match.
    // The command name is surrounded by parens and may contain spaces.
    let (cmd_start, cmd_end) = (stat.find('(')? + 1, stat.rfind(')')?);
    if filter.map_or(false, |f| f != &stat[cmd_start..cmd_end]) {
        return None;
    }
    // Parse start time
    let start_time = i64::from_str(stat[cmd_end + 1..].split(' ').nth(20)?).ok()?;
    // Parse arguments
    let mut cmdline = String::new();
    File::open(entry.path().join("cmdline")).ok()?.read_to_string(&mut cmdline).ok()?;
    cmdline = cmdline.replace("\0", " ").trim().into();
    // Done
    Some(Info { cmdline, start: time_ref + (start_time / clocktick) as i64, pid })
}

/// Get command name, arguments, start time, and pid for all processes.
pub fn get_all_info(filter: Option<&str>) -> Result<Vec<Info>, io::Error> {
    // clocktick and time_ref are needed to interpret stat.start_time. time_ref should correspond to
    // the system boot time; not sure why it doesn't, but it's still usable as a reference.
    let clocktick = sysconf(SysconfVariable::ScClkTck).unwrap() as i64;
    let mut uptimestr = String::new();
    File::open("/proc/uptime")?.read_to_string(&mut uptimestr)?;
    let uptime = i64::from_str(uptimestr.split('.').next().unwrap()).unwrap();
    let time_ref = epoch_now() - uptime;
    // Now iterate through /proc/<pid>
    let mut ret: Vec<Info> = Vec::new();
    for entry in read_dir("/proc/")? {
        if let Some(i) = get_proc_info(filter, &entry?, clocktick, time_ref) {
            ret.push(i)
        }
    }
    Ok(ret)
}


#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use regex::Regex;
    use std::{collections::BTreeMap, process::Command};

    use crate::proces::*;

    fn parse_ps_time(s: &str) -> i64 {
        DateTime::parse_from_str(&format!("{} +0000", s), "%b %d %T %Y %z")//we run ps with TZ=UTC
            .expect(&format!("Cannot parse {}", s))
            .timestamp()
    }

    #[test] #[rustfmt::skip]
    fn start_time() {
        // First get the system's process start times using our implementation
        let mut info: BTreeMap<i32,(String,Option<i64>,Option<i64>)> = //pid => (cmd, rust_time, ps_time)
            get_all_info(None)
            .unwrap()
            .iter()
            .fold(BTreeMap::new(), |mut a,i| {a.insert(i.pid, (i.cmdline.clone(),Some(i.start),None)); a});
        // Then get them using the ps implementation (merging them into the same data structure)
        let ps_start = epoch_now();
        let re = Regex::new("^ *([0-9]+) [A-Za-z]+ ([a-zA-Z0-9: ]+)$").unwrap();
        let cmd = Command::new("ps").env("TZ", "UTC")
                                    .env("LC_ALL", "C") // Use a consistent format for datetimes
                                    .args(&["-o",
                                            "pid,lstart", // Output pid and start time
                                            "-ax", // All processes including those "not associated with a terminal"
                                            "--no-header"]) // No headers
                                    .output()
                                    .expect("failed to execute ps");
        for lineres in cmd.stdout.lines() {
            if let Ok(line) = lineres {
                match re.captures(&line) {
                    Some(c) => {
                        let pid = c.get(1).unwrap().as_str().parse::<i32>().unwrap();
                        let time = parse_ps_time(c.get(2).unwrap().as_str());
                        if let Some((comm, t, None)) =
                            info.insert(pid, ("?".into(), None, Some(time)))
                        {
                            info.insert(pid, (comm, t, Some(time)));
                        }
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
            match times {
                (ref c,Some(t), None) =>                           {e+=1; println!("WARN {:>20} {:>7} {}: disappeared after rust run", c, pid, fmt_time(t));},
                (ref c,None, Some(t)) if t >= ps_start -1 =>       {e+=1; println!("WARN {:>20} {:>7} {}: appeared right after rust run", c, pid, fmt_time(t));},
                (ref c,None, Some(t)) =>                           {e+=10;println!("ERR  {:>20} {:>7} {}: seen by ps but not by rust", c, pid, fmt_time(t));},
                (ref c,Some(tr), Some(tp)) if tr == tp =>          {e+=0; println!("OK   {:>20} {:>7} {}: same time", c, pid, fmt_time(tr));},
                (ref c,Some(tr), Some(tp)) if (tr-tp).abs() < 2 => {e+=0; println!("IGN  {:>20} {:>7} {}: {} secs diff {}", c, pid, fmt_time(tr), tr-tp, fmt_time(tp));},
                (ref c,Some(tr), Some(tp)) if (tr-tp).abs() < 5 => {e+=1; println!("WARN {:>20} {:>7} {}: {} secs diff {}", c, pid, fmt_time(tr), tr-tp, fmt_time(tp));},
                (ref c,Some(tr), Some(tp)) =>                      {e+=5; println!("ERR  {:>20} {:>7} {}: {} secs diff {}", c, pid, fmt_time(tr), tr-tp, fmt_time(tp));},
                (ref c,None, None) =>                              {e+=10;println!("ERR  {:>20} {:>7}: no times", c, pid);},
            }
        }
        assert!(e < 10, "Got failure score of {}", e);
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
            let i = Info { pid, cmdline: s[..cmdlen].to_string(), start: 0 };
            let f = format!("{1:.0$}", precision, i);
            assert!(precision < 10 || f.len() <= precision, "{} <= {}", f.len(), precision);
            assert_eq!(f, out);
        }
    }
}
