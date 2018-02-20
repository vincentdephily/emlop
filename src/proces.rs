//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use std::fs;
use std::io;
use std::io::prelude::*;
use sysconf::raw::{sysconf, SysconfVariable};

use ::*;

#[derive(Debug)]
pub struct Info {
    pub comm: String,
    pub cmdline: String,
    pub start: i64,
    pub pid: i32,
}

/// Get command name, arguments, start time, and pid for one process.
fn get_proc_info(filter: Option<&str>, entry: fs::DirEntry, clocktick: i64, time_ref: i64) -> Option<Info> {
    // Parse pid.
    // At this stage we expect `entry` to not always correspond to a process.
    let pid = i32::from_str(&entry.file_name().to_string_lossy()).ok()?;
    // Parse command name and start time.
    // See linux/Documentation/filesystems/proc.txt Table 1-4: Contents of the stat files.
    // The command name is surrounded by parens and may contain spaces. Parsing will fail if it contains a closing paren followed by a space.
    let mut statstr = String::new();
    fs::File::open(entry.path().join("stat")).ok()?.read_to_string(&mut statstr).ok()?;
    let statfields: Vec<&str> = statstr.split(' ').collect();
    let mut parse_offset = 0;
    let mut comm = statfields[1].trim_left_matches('(').to_string();
    while !comm.ends_with(")") {
        parse_offset += 1;
        comm = format!("{} {}", comm, statfields[1 + parse_offset]);
    }
    comm = comm.trim_right_matches(')').to_string();
    let start_time = i64::from_str(statfields[21 + parse_offset]).ok()?;
    // Bail out now if the command name doesn't match.
    if filter.map_or(false, |f| f != comm) {
        return None;
    }
    // Parse arguments
    let mut cmdline = String::new();
    fs::File::open(entry.path().join("cmdline")).ok()?.read_to_string(&mut cmdline).ok()?;
    cmdline = cmdline.replace("\0", " ").trim().into();
    // Done
    Some(Info{comm: comm,
              cmdline: cmdline,
              start: time_ref + (start_time / clocktick) as i64,
              pid: pid,
    })
}

/// Get command name, arguments, start time, and pid for all processes.
pub fn get_all_info(filter: Option<&str>) -> Result<Vec<Info>, io::Error> {
    // clocktick and time_ref are needed to interpret stat.start_time. time_ref should correspond to
    // the system boot time; not sure why it doesn't, but it's still usable as a reference.
    let clocktick = sysconf(SysconfVariable::ScClkTck).unwrap() as i64;
    let mut uptimestr = String::new();
    fs::File::open("/proc/uptime")?.read_to_string(&mut uptimestr)?;
    let uptime = i64::from_str(uptimestr.split('.').nth(0).unwrap()).unwrap();
    let time_ref = epoch_now() - uptime;
    // Now iterate through /proc/<pid>
    let mut ret: Vec<Info> = Vec::new();
    for entry in fs::read_dir("/proc/")? {
        let entry = entry?;
        if let Some(i) = get_proc_info(filter, entry, clocktick, time_ref) {
            ret.push(i)
        }
    }
    Ok(ret)
}


#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use regex::Regex;
    use std::collections::BTreeMap;
    use std::process::Command;

    use ::*;
    use proces::*;

    fn parse_ps_time(s: &str) -> i64 {
        DateTime::parse_from_str(&format!("{} +0000", s), "%b %d %T %Y %z")//we run ps with TZ=UTC
            .expect(&format!("Cannot parse {}", s))
            .timestamp()
    }

    #[test]
    fn start_time() {
        // First get the system's process start times using our implementation
        let mut info: BTreeMap<i32,(String,Option<i64>,Option<i64>)> = //pid => (cmd, rust_time, ps_time)
            get_all_info(None)
            .unwrap()
            .iter()
            .fold(BTreeMap::new(), |mut a,i| {a.insert(i.pid, (i.comm.clone(),Some(i.start),None)); a});
        // Then get them using the ps implementation (merging them into the same data structure)
        let ps_start = epoch_now();
        let re = Regex::new("^ *([0-9]+) [A-Za-z]+ ([a-zA-Z0-9: ]+)$").unwrap();
        let cmd = Command::new("ps")
            .env("TZ", "UTC").env("LC_ALL", "C") // Use a consistent format for datetimes
            .args(&["-o", "pid,lstart", // Output pid and start time
                    "-ax", // All processes including those "not associated with a terminal"
                    "-h"]) // No headers
            .output()
            .expect("failed to execute ps");
        for lineres in cmd.stdout.lines() {
            if let Ok(line) = lineres {
                match re.captures(&line) {
                    Some(c) => {
                        let pid = c.get(1).unwrap().as_str().parse::<i32>().unwrap();
                        let time = parse_ps_time(c.get(2).unwrap().as_str());
                        if let Some((comm,t,None)) = info.insert(pid,("?".into(),None,Some(time))) {
                            info.insert(pid,(comm,t,Some(time)));
                        }
                    },
                    None => assert!(false, "Couldn't parse {}", line),
                }
            }
        }
        // Check the results. For debugging purposes it's usefull to print everything and only
        // assert at the end. Also some cases are considered soft errors and only make the whole
        // test fail if they happen a lot.
        assert!(info.len() > 10, "Only {} processes found", info.len());
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
}
