//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use crate::{config::*, table::Disp, *};
use anyhow::{ensure, Context};
use libc::pid_t;
use std::{collections::BTreeMap,
          fs::{read_dir, DirEntry, File},
          io::prelude::*,
          path::PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum ProcKind {
    Emerge,
    Python,
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

/// Like `Path.file_name()`, but less likely to interpret package categ/name as files
fn approx_filename(s: &str) -> Option<usize> {
    if s.chars().all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '/')) {
        s.rfind('/')
    } else {
        None
    }
}

pub struct FmtProc<'a>(/// process
                       pub &'a Proc,
                       /// Indent
                       pub usize,
                       /// Width
                       pub usize);
impl Disp for FmtProc<'_> {
    fn out(&self, buf: &mut Vec<u8>, gc: &Conf) -> usize {
        let FmtProc(Proc { cmdline, pid, .. }, indent, width) = *self;
        let (cnt, clr) = (gc.cnt.val, gc.clr.val);

        // Skip path and interpreter from command line
        let mut cmdstart = 0;
        if let Some(z1) = cmdline.find('\0') {
            if let Some(f1) = approx_filename(&cmdline[..z1]) {
                cmdstart = f1 + 1;
            }
            if let Some(z2) = cmdline[z1 + 1..].find('\0') {
                if let Some(f2) = approx_filename(&cmdline[z1 + 1..z1 + 1 + z2]) {
                    cmdstart = z1 + f2 + 2;
                }
            }
        }
        let cmd = cmdline[cmdstart..].replace(|c: char| c.is_control(), " ");
        let cmd = cmd.trim();

        // Figure out how much space we have
        let pidlen = pid.max(&1).ilog10() as usize + 2 * indent + 1;
        let cmdcap = width.saturating_sub(pidlen + 1);

        // Output it
        if cmdcap >= cmd.len() {
            wtb!(buf, "{cnt}{pid:pidlen$}{clr} {cmd}");
            pidlen + 1 + cmd.len()
        } else if cmdcap > 3 {
            wtb!(buf, "{cnt}{pid:pidlen$}{clr} ...{}", &cmd[(cmd.len() - cmdcap + 3)..]);
            pidlen + 1 + cmdcap
        } else {
            wtb!(buf, "{cnt}{pid:pidlen$}{clr} ...");
            pidlen + 4
        }
    }
}

/// Get command name, arguments, start time, and pid for one process.
fn get_proc(entry: &DirEntry,
            clocktick: i64,
            time_ref: i64,
            tmpdirs: &mut Vec<PathBuf>)
            -> Option<Proc> {
    // Parse pid.
    // At this stage we expect `entry` to not always correspond to a process.
    let pid = i32::from_str(&entry.file_name().to_string_lossy()).ok()?;
    // See linux/Documentation/filesystems/proc.rst Table 1-4: Contents of the stat files.
    let mut stat = String::new();
    File::open(entry.path().join("stat")).ok()?.read_to_string(&mut stat).ok()?;
    // Parse command name (it's surrounded by parens and may contain spaces)
    // If it's emerge, look for portage tmpdir in its fds
    let (cmd_start, cmd_end) = (stat.find('(')? + 1, stat.rfind(')')?);
    let kind = if &stat[cmd_start..cmd_end] == "emerge" {
        extend_tmpdirs(entry.path(), tmpdirs);
        ProcKind::Emerge
    } else if stat[cmd_start..cmd_end].starts_with("python") {
        ProcKind::Python
    } else {
        ProcKind::Other
    };
    // Parse parent pid and start time
    let mut fields = stat[cmd_end + 1..].split(' ');
    let ppid = i32::from_str(fields.nth(2)?).ok()?;
    let start_time = i64::from_str(fields.nth(17)?).ok()?;
    // Parse arguments
    let mut cmdline = String::new();
    File::open(entry.path().join("cmdline")).ok()?.read_to_string(&mut cmdline).ok()?;
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

pub type ProcList = BTreeMap<pid_t, Proc>;

/// Get command name, arguments, start time, and pid for all processes.
pub fn get_all_proc(tmpdirs: &mut Vec<PathBuf>) -> ProcList {
    get_all_proc_result(tmpdirs).unwrap_or_else(|e| {
                                    log_err(e);
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
    let mut uptimestr = String::new();
    File::open("/proc/uptime").context("Opening /proc/uptime")?
                              .read_to_string(&mut uptimestr)
                              .context("Reading /proc/uptime")?;
    let uptime = i64::from_str(uptimestr.split('.').next().unwrap()).unwrap();
    let time_ref = epoch_now() - uptime;
    // Now iterate through /proc/<pid>
    let mut ret: BTreeMap<pid_t, Proc> = BTreeMap::new();
    for entry in read_dir("/proc/").context("Listing /proc/")? {
        if let Some(p) = get_proc(&entry?, clocktick, time_ref, tmpdirs) {
            ret.insert(p.pid, p);
        }
    }
    Ok(ret)
}


#[cfg(test)]
pub mod tests {
    use super::{config::Conf, *};
    use regex::Regex;
    use std::{collections::BTreeMap, process::Command};
    use time::{macros::format_description, PrimitiveDateTime};

    /// Helper to create a process list
    pub fn procs(procs: &[(ProcKind, &str, pid_t, pid_t)]) -> ProcList {
        BTreeMap::from_iter(procs.into_iter().map(|p| {
                                                 (p.2,
                                                  Proc { kind: p.0,
                                                         cmdline: p.1.into(),
                                                         start: p.2 as i64,
                                                         pid: p.2,
                                                         ppid: p.3 })
                                             }))
    }

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

    /// FmtProc should try shorten (elipsis at start) the command line when ther is no space
    #[test]
    fn proc_width() {
        let conf = Conf::from_str(&format!("emlop p --color=n"));
        let t: Vec<_> = vec![// Here we have enough space
                             (1, "1", "1 1"),
                             (1, "12", "1 12"),
                             (1, "12345678", "1 12345678"),
                             (12, "1234567", "12 1234567"),
                             (123, "123456", "123 123456"),
                             (1234, "12345", "1234 12345"),
                             // Running out of space, but we can display part of it
                             (1, "1234567890", "1 ...67890"),
                             (12345, "1234567890", "12345 ...0"),
                             // Capacity is way too small, use elipsis starting at 4 chars
                             (1234567, "123", "1234567 ..."),
                             (123456, "123", "123456 123"),];
        for (pid, cmd, out) in t.into_iter() {
            let mut buf = vec![];
            let p = Proc { kind: ProcKind::Other, pid, ppid: 1, cmdline: cmd.into(), start: 0 };
            FmtProc(&p, 0, 10).out(&mut buf, &conf);
            assert_eq!(&String::from_utf8(buf).unwrap(),
                       out,
                       "got left expected right {pid} {cmd:?}");
        }
    }

    /// FmtProc should rewrite commands
    #[test]
    fn proc_cmdline() {
        let conf = Conf::from_str(&format!("emlop p --color=n"));
        let t: Vec<_> =
            vec![("foo\0bar", "1 foo bar"),
                 ("foo\0bar\0", "1 foo bar"),
                 ("/usr/bin/bash\0toto", "1 bash toto"),
                 ("/usr/bin/bash\0toto\0", "1 bash toto"),
                 ("/usr/bin/bash\0toto\0--arg", "1 bash toto --arg"),
                 ("/usr/bin/bash\0/path/to/toto\0--arg", "1 toto --arg"),
                 ("bash\0/usr/lib/portage/python3.12/ebuild.sh\0unpack\0", "1 ebuild.sh unpack"),
                 ("[foo/bar-0.1.600] sandbox\0/path/to/ebuild.sh\0unpack\0", "1 ebuild.sh unpack"),
                 ("[foo/bar-0.1.600] sandbox\0blah\0", "1 [foo/bar-0.1.600] sandbox blah"),
                 ("/bin/foo\0\0", "1 foo")];
        for (cmd, out) in t.into_iter() {
            let mut buf = vec![];
            let p = Proc { kind: ProcKind::Other, pid: 1, ppid: 1, cmdline: cmd.into(), start: 0 };
            FmtProc(&p, 0, 100).out(&mut buf, &conf);
            assert_eq!(&String::from_utf8(buf).unwrap(), out, "got left expected right {cmd:?}");
        }
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
