//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use crate::Pkg;
use anyhow::{Context, Error, ensure};
use atoi::atoi;
use libc::pid_t;
use log::{debug, error, trace};
use std::{collections::BTreeMap,
          fs::{DirEntry, File, read_dir, read_to_string},
          io::prelude::*,
          path::PathBuf,
          str::FromStr,
          time::Instant};
use time::Timestamp;

#[derive(Debug, Clone, Copy)]
/// Portage process kind
pub enum ProcKind {
    /// Main portage `emerge` process
    ///
    /// One of those is the initial emerge command, and some of those may have a build.log open,
    /// which tels us where tmpdirs is.
    /// `
    /// emerge /usr/lib/python-exec/python3.11/emerge -Ov1 dummybuild
    /// `
    Emerge,
    /// Helper portage `sandbox` process
    ///
    /// Tells us the current (un)merging ebuild and stage (depends on portage FEATURES=sandbox,
    /// which should be the case for almost all users).
    /// `
    /// python3.11 /usr/lib/portage/python3.11/pid-ns-init 250 250 250 18 0,1,2 /usr/bin/sandbox
    /// [app-portage/dummybuild-0.1.600] sandbox /usr/lib/portage/python3.11/ebuild.sh unpack`
    /// `
    Sandbox,
    /// Other process, possibly not related to portage
    Other,
}

#[derive(Debug)]
/// System process info
pub struct Proc {
    pub kind: ProcKind,
    pub cmdline: String,
    pub start: i64,
    pub pid: pid_t,
    pub ppid: pid_t,
}
impl Proc {
    /// Fill `Proc` struct for one process
    pub fn try_new(entry: &DirEntry, clocktick: i64, time_ref: i64) -> Option<Self> {
        // Parse pid.
        // At this stage we expect `entry` to not always correspond to a process.
        let pid = i32::from_str(&entry.file_name().to_string_lossy()).ok()?;
        // See linux/Documentation/filesystems/proc.rst Table 1-4: Contents of the stat files.
        let stat = read_to_string(entry.path().join("stat")).ok()?;
        // Parse command name (it's surrounded by parens and may contain spaces)
        // If it's emerge, look for portage tmpdir in its fds
        let (cmd_start, cmd_end) = (stat.find('(')? + 1, stat.rfind(')')?);
        let kind = if &stat[cmd_start..cmd_end] == "emerge" {
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
        Some(Self { kind, cmdline, start: time_ref + start_time / clocktick, pid, ppid })
    }
}

/// Info about current emerge process
pub struct EmergeInfo {
    /// [Proc] info of every system process
    pub procs: BTreeMap<pid_t, Proc>,
    /// Pid of the initial emerge command(s), the root of the build tree(s)
    pub roots: Vec<pid_t>,
    /// Startup timestamp of the oldest root
    ///
    /// [i64::MAX] if no root is found
    pub start: i64,
    /// Packages currently being built by a process
    pub pkgs: Vec<Pkg>,
}
impl EmergeInfo {
    /// Builds the [EmergeInfo] struct, and update `tmpdirs` in-place
    ///
    /// This function always succeeds: failure to read the process list results in a log and an
    /// empty struct.
    pub fn new(tmpdirs: &mut Vec<PathBuf>) -> Self {
        let now = Instant::now();
        let r = Self::try_new(tmpdirs).unwrap_or_else(|e| {
                                          match e.source() {
                                              Some(s) => error!("{e}: {s}"),
                                              None => error!("{e}"),
                                          };
                                          Self::default()
                                      });
        debug!("Found {} procs ({} roots, {} pkgs) in {:?}",
               r.procs.len(),
               r.roots.len(),
               r.pkgs.len(),
               now.elapsed());
        r
    }

    /// Return direct children of given pid
    pub fn children_of(&self, pid: pid_t) -> impl Iterator<Item = &Proc> {
        self.procs.values().filter(move |p| p.ppid == pid)
    }

    /// Count all children of given pid
    pub fn count(&self, pid: pid_t) -> u32 {
        1 + self.children_of(pid).map(|c| self.count(c.pid)).sum::<u32>()
    }

    fn default() -> Self {
        Self { procs: BTreeMap::new(), roots: vec![], start: i64::MAX, pkgs: vec![] }
    }

    fn try_new(tmpdirs: &mut Vec<PathBuf>) -> Result<Self, Error> {
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
        let mut ret = Self::default();
        for entry in read_dir("/proc/").context("Listing /proc/")?.filter_map(Result::ok) {
            if let Some(proc) = Proc::try_new(&entry, clocktick, time_ref) {
                ret.add_proc(proc, &entry, tmpdirs);
            }
        }
        ret.reduce_roots();
        Ok(ret)
    }

    /// Update self with info from [Proc]
    fn add_proc(&mut self, proc: Proc, entry: &DirEntry, tmpdirs: &mut Vec<PathBuf>) {
        match proc.kind {
            ProcKind::Emerge => {
                self.start = std::cmp::min(self.start, proc.start);
                self.roots.push(proc.pid);
                Self::extend_tmpdirs(entry.path(), tmpdirs);
            },
            ProcKind::Sandbox => {
                if let Some(a) = proc.cmdline.find("] sandbox\0")
                   && let Some(b) = proc.cmdline[..a].rfind("[")
                   && let Some(p) = Pkg::try_new(&proc.cmdline[(b + 1)..a], false)
                {
                    self.pkgs.push(p);
                }
            },
            ProcKind::Other => (),
        }
        self.procs.insert(proc.pid, proc);
    }

    /// Remove roots that  one of their parent is already a root
    fn reduce_roots(&mut self) {
        self.roots.retain(|&r| {
                      let mut proc = self.procs.get(&r).expect("Root not in procs");
                      while let Some(p) = self.procs.get(&proc.ppid) {
                          if matches!(p.kind, ProcKind::Emerge) {
                              trace!("Removing root {}: grandchild of {}", r, p.pid);
                              return false;
                          }
                          proc = p;
                      }
                      true
                  });
    }

    /// Find tmpdir by looking for "build.log" in the process fds, and add it to the provided vector
    fn extend_tmpdirs(procpath: PathBuf, tmpdirs: &mut Vec<PathBuf>) {
        if let Ok(entries) = read_dir(procpath.join("fd")) {
            for d in entries.filter_map(|e| {
                                let p = e.ok()?.path().canonicalize().ok()?;
                                if p.file_name()? != "build.log" {
                                    return None;
                                }
                                let d = p.parent()?.parent()?.parent()?.parent()?.parent()?;
                                debug!("Tmpdir {:?} found in {:?}", d, procpath);
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

    #[cfg(feature = "test-helpers")]
    /// Create [EmergeInfo] from a list of (kind, cmdline, pid, ppid) tuples
    pub fn mock<const N: usize>(procs: [(ProcKind, &str, pid_t, pid_t); N]) -> Self {
        let mut ret = Self::default();
        let entry: DirEntry = read_dir("/").unwrap().next().unwrap().unwrap();
        let mut tmpdirs = vec![];
        for p in procs {
            let proc =
                Proc { kind: p.0, cmdline: p.1.into(), start: p.2 as i64, pid: p.2, ppid: p.3 };
            ret.add_proc(proc, &entry, &mut tmpdirs);
        }
        ret.reduce_roots();
        ret
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
        let mut info = EmergeInfo::new(&mut vec![])
            .procs
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

    /// Check that get_emerge() finds the expected roots
    #[test]
    fn get_emerge_roots() {
        let _ = env_logger::try_init();
        let einfo = EmergeInfo::mock([(ProcKind::Emerge, "a", 1, 0),
                                      (ProcKind::Other, "a.a", 2, 1),
                                      (ProcKind::Emerge, "a.a.b", 3, 2),
                                      (ProcKind::Other, "b", 4, 0),
                                      (ProcKind::Emerge, "b.a", 5, 6),
                                      (ProcKind::Emerge, "b.a", 6, 4),
                                      (ProcKind::Other, "b.a.a", 7, 5)]);
        assert_eq!(einfo.roots, vec![1, 6]);
    }
}

#[cfg(feature = "unstable")]
#[cfg(test)]
mod bench {
    extern crate test;

    #[bench]
    /// Bench listing all processes
    fn get_procs(b: &mut test::Bencher) {
        b.iter(move || {
             super::EmergeInfo::new(&mut vec![]);
         });
    }
}
