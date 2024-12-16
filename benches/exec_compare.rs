#!/usr/bin/env scriptisto

//! Quick script to benchmark *lop implementations.

//! scriptisto-begin
//! script_src: src/main.rs
//! build_cmd: cargo build --release
//! target_bin: ./target/release/exec_compare
//! files:
//!  - path: Cargo.toml
//!    content: |
//!     package = { name = "exec_compare", version = "0.1.0", edition = "2021"}
//!     [dependencies]
//!     clap = {version = "4.5.23", features = ["string"]}
//!     stats-cli = "3.0.1"
//!     tabwriter = "1.4.0"
//!     rand = "0.8.5"
//! scriptisto-end

use clap::{value_parser, Arg, ArgAction::*, Command as ClapCmd};
use inc_stats::*;
use rand::prelude::SliceRandom;
use std::{collections::BTreeMap,
          fs::File,
          io::{stderr, Write},
          process::{Command, Stdio},
          time::Instant};
use tabwriter::TabWriter;

fn main() {
    // Test definitions: (test suite, program name, program args, stdin)
    #[rustfmt::skip]
    let defs: Vec<(&str,&str,&[&str],Option<&str>)> = vec![
        // Show version, useful to bench startup cost
        ("v", "genlop", &["-v"], None),
        ("v", "qlop",   &["-V"], None),
        ("v", "emlop",  &["-V"], None),
        // Minimal command to read the first result (no equivalent in genlop/qlop)
        ("start", "emlop",  &["-F","{emerge.log}","l","-N"],  None),
        // Minimal "show all merges" command (genlop adds version, emlop adds version+duration)
        ("l", "genlop", &["-f","{emerge.log}","-l"], None),
        ("l", "qlop",   &["-f","{emerge.log}","-m"], None),
        ("l", "emlop",  &["-F","{emerge.log}","l"],  None),
        // Show all merges+unmegres with version and duration
        ("ltmu", "genlop", &["-f","{emerge.log}","-lut"],     None),
        ("ltmu", "qlop",   &["-f","{emerge.log}","-muUvt"],   None),
        ("ltmu", "emlop",  &["-F","{emerge.log}","l","-smu"], None),
        // Show sync history
        ("s", "genlop", &["-f","{emerge.log}","-r"],      None),
        ("s", "qlop",   &["-f","{emerge.log}","-st"],     None),
        ("s", "emlop",  &["-F","{emerge.log}","l","-ss"], None),
        // Show last sync
        ("ls", "genlop", &["-f","{emerge.log}","-r","--date","2020-10-08"], None),
        ("ls", "qlop",   &["-f","{emerge.log}","-stl"],                     None),
        ("ls", "emlop",  &["-F","{emerge.log}","l","-ss","-n"],             None),
        // Read only part of a file
        ("ld1", "genlop", &["-f","{emerge.log}","-l", "--date","2019-02-01","--date","2019-02-28"], None),
        ("ld1", "qlop",   &["-f","{emerge.log}","-mv","--date","2019-02-01","--date","2019-02-28"], None),
        ("ld1", "emlop",  &["-F","{emerge.log}","l",  "--from","2019-02-01","--to",  "2019-02-28"], None),
        ("ld2", "genlop", &["-f","{emerge.log}","-l", "--date","2020-10-01","--date","2020-10-31"], None),
        ("ld2", "qlop",   &["-f","{emerge.log}","-mv","--date","2020-10-01","--date","2020-10-31"], None),
        ("ld2", "emlop",  &["-F","{emerge.log}","l",  "--from","2020-10-01","--to",  "2020-10-31"], None),
        ("ldl", "qlop",   &["-f","{emerge.log}","-mv","--lastmerge"], None),
        ("ldl", "emlop",  &["-F","{emerge.log}","l","--from=1c"],   None),
        // Force/prevent color output
        ("lc", "qlop",   &["-f","{emerge.log}","-mv","--color"],   None),
        ("lc", "emlop",  &["-F","{emerge.log}","l","--color=y"],   None),
        ("ln", "genlop", &["-f","{emerge.log}","-l","-n"],         None),
        ("ln", "qlop",   &["-f","{emerge.log}","-mv","--nocolor"], None),
        ("ln", "emlop",  &["-F","{emerge.log}","l","--color=n"],   None),
        // Simple package merge log
        ("egcc", "genlop", &["-f","{emerge.log}","gcc"],          None),
        ("egcc", "qlop",   &["-f","{emerge.log}","-m","gcc"],     None),
        ("egcc", "emlop",  &["-F","{emerge.log}","l","-e","gcc"], None),
        ("rgcc", "emlop",  &["-F","{emerge.log}","l","gcc"],      None),
        // Multiple packages merge log
        ("emany", "genlop", &["-f","{emerge.log}","llvm","emacs", "gcc"],          None),
        ("emany", "qlop",   &["-f","{emerge.log}","-m","llvm","emacs", "gcc"],     None),
        ("emany", "emlop",  &["-F","{emerge.log}","l","-e","llvm","emacs", "gcc"], None),
        ("rmany", "emlop",  &["-F","{emerge.log}","l","llvm","emacs", "gcc"],      None),
        // Version+duration package merge+unmerge log (genlop ignores filter when showig unmerges)
        ("tgcc", "qlop",   &["-f","{emerge.log}","-tvmuU","gcc"],        None),
        ("tgcc", "emlop",  &["-F","{emerge.log}","l","-smu","-e","gcc"], None),
        // Predict current merge(s)
        ("c", "genlop", &["-c"], None),
        ("c", "qlop",   &["-r"], None),
        ("c", "emlop",  &["p"],  None),
        // Predict merge list
        ("pgcc", "genlop", &["-f","{emerge.log}","-p"], Some("benches/emerge-p.gcc.out")),
        ("pgcc", "emlop",  &["-F","{emerge.log}","p"],  Some("benches/emerge-p.gcc.out")),
        ("pqt",  "genlop", &["-f","{emerge.log}","-p"], Some("benches/emerge-p.qt.out")),
        ("pqt",  "emlop",  &["-F","{emerge.log}","p"],  Some("benches/emerge-p.qt.out")),
        ("pkde", "genlop", &["-f","{emerge.log}","-p"], Some("benches/emerge-p.kde.out")),
        ("pkde", "emlop",  &["-F","{emerge.log}","p"],  Some("benches/emerge-p.kde.out")),
        // Show all info about a specific package
        ("igcc", "genlop", &["-f","{emerge.log}","-i","gcc"],     None),
        ("igcc", "qlop",   &["-f","{emerge.log}","-c","gcc"],     None),
        ("igcc", "emlop",  &["-F","{emerge.log}","s","gcc","-e"], None),
        // Show overall stats
        ("st", "emlop",  &["-F","{emerge.log}","s","-sa"], None),
    ];

    // CLI definition
    let mut allprogs: Vec<&str> = defs.iter().map(|&(_, p, _, _)| p).collect();
    allprogs.sort();
    allprogs.dedup();
    let mut allsets: Vec<String> = defs.iter().map(|&(s, _, _, _)| s.into()).collect();
    allsets.sort();
    allsets.dedup();
    let allsets_str = allsets.join(",");
    let cli = ClapCmd::new("emlop-bench")
        .about("Quick script to benchmark *lop implementations.")
        .after_help("All benchmarks are biased. Some tips to be less wrong:\n\
                     * Make your system is as idle as possible, shutdown unneeded apps \
                     (browser, im, cron...).\n\
                     * Don't compare numbers collected at different times or on different \
                     machines.\n\
                     * Look at all indicators, not just the mean.\n\
                     * The terminal emulator's speed makes a big difference. \
                     Reduce the scroll buffer size and check performance-related settings.\n\
                     * Use -n option (redirect to /dev/null) to ignore terminal overhead.\n\
                     * Pipe to cat to disable colors (see also color-specific sets).")
        .arg(Arg::new("program")
             .help("Programs to test, formated as 'NAME[:PATH]'")
             .long_help("Programs to test, formated as 'NAME[:PATH]'\n  \
                         name can be abbreviated, alternative path can be provided.\n  \
                         eg 'emlop e:target/release/emlop q'")
             .num_args(1..)
             .default_value("emlop"))
        .arg(Arg::new("sets")
             .help("Test sets")
             .short('s')
             .num_args(1..)
             .use_value_delimiter(true)
             .value_parser(allsets)
             .hide_possible_values(true)
             .default_value(allsets_str))
        .arg(Arg::new("runs")
             .help("Number of iterations")
             .short('r')
             .num_args(1)
             .value_parser(value_parser!(i32))
             .default_value("10"))
        .arg(Arg::new("buckets")
             .help("Number of histogram buckets")
             .short('b')
             .num_args(1)
             .value_parser(value_parser!(i32))
             .default_value("5"))
        .arg(Arg::new("nullout")
             .short('n')
             .action(SetTrue)
             .help("Send test program outputs to /dev/null"))
        .arg(Arg::new("out")
             .short('o')
             .num_args(1)
             .help("Write results to file instead of stderr"))
        .arg(Arg::new("logfile")
             .help("Emerge log file")
             .short('f')
             .num_args(1)
             .default_value("./benches/emerge.log"))
        .arg(Arg::new("conf")
             .long("conf")
             .num_args(1)
             .default_value("")
             .help("Let emlop load its config file"))
        .get_matches();

    // CLI parsing
    let runs = *cli.get_one("runs").unwrap();
    let buckets = *cli.get_one("buckets").unwrap();
    let progs: Vec<&String> = cli.get_many("program").unwrap().collect();
    let sets: Vec<&String> = cli.get_many("sets").unwrap().collect();
    let nullout = cli.get_flag("nullout");
    let logfile: &String = cli.get_one("logfile").unwrap();

    // Construct the test list.
    let mut tests = Vec::<(String, &str, Vec<&str>, Option<&str>)>::new();
    for p in progs.iter() {
        // Resolve this program's name and path
        let (p1, p2) = p.split_at(p.find(':').unwrap_or(p.len()));
        let mut pmatch = allprogs.iter().filter(|s| s.starts_with(p1));
        let pname = match (pmatch.next(), pmatch.next()) {
            (Some(s), None) => s,
            _ => {
                eprintln!("{:?} should match exactly one of {:?}", p1, allprogs);
                ::std::process::exit(1);
            },
        };
        let ppath = if p2.is_empty() { pname } else { p2.trim_start_matches(':') };

        // Add matching tests to test vector
        let mut found = Vec::new();
        for &(set, prg, args, si) in &defs {
            if &prg == pname && sets.contains(&&set.to_string()) {
                found.push(set);
                let args: Vec<&str> =
                    args.iter().map(|&s| if s == "{emerge.log}" { &logfile } else { s }).collect();
                let name = format!("{set}\t{ppath} {}{}",
                                   args.join(" "),
                                   si.map_or(String::new(), |s| format!(" < {}", s)));
                for _ in 0..runs {
                    tests.push((name.clone(), ppath, args.clone(), si));
                }
            }
        }
        sets.iter()
            .filter(|s| !found.contains(&s.as_str()))
            .for_each(|s| eprintln!("Test {:?} not defined for {:?}.", s, pname));
    }

    // Load emerge.log in the OS cache
    assert_eq!(0,
               Command::new("cat").arg(&logfile)
                                  .stdout(Stdio::null())
                                  .status()
                                  .unwrap()
                                  .code()
                                  .unwrap());

    // Run the tests and collect the results
    std::env::set_var("EMLOP_CONFIG", cli.get_one::<String>("conf").unwrap());
    tests.shuffle(&mut rand::thread_rng());
    let mut n = tests.len();
    let mut times: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (name, bin, args, stdin) in &tests {
        match nullout {
            true => eprint!("\r{} ", n),
            false => eprintln!(">>>>>> {} {}", n, &name),
        }
        n -= 1;
        let si =
            stdin.map_or(Stdio::inherit(), |f| File::open(f).expect(&format!("open {f}")).into());
        let so = if nullout { Stdio::null() } else { Stdio::inherit() };
        let err = &format!("Couldn't run {} {:?}", bin, args);
        let mut cmd = Command::new(bin);
        let start = Instant::now();
        cmd.args(args).stdin(si).stdout(so).status().expect(err);
        let elapsed = start.elapsed().as_millis() as f64;
        times.entry(name.clone()).or_insert(vec![]).insert(0, elapsed);
        times.entry(format!("*\t{bin}")).or_insert(vec![]).insert(0, elapsed);
    }

    // Output the results
    let mut out = Out::try_new(cli.get_one("out")).unwrap();
    for (key, vals) in times {
        let ss: SummStats<f64> = vals.iter().cloned().collect();
        let pc: Percentiles<f64> = vals.iter().cloned().collect();
        let (min, max) = (ss.min().unwrap(), ss.max().unwrap());
        let step = (max - min + 0.001) / std::cmp::min(buckets, runs) as f64;
        let mut hist: BTreeMap<u64, u64> = BTreeMap::new();
        vals.into_iter()
            .map(|v| ((((v - min) / step).floor() * step) + min) as u64)
            .for_each(|v| *hist.entry(v).or_insert(0) += 1);
        let hist = hist.iter().map(|(k, v)| format!("{}:{}", k, v)).collect::<Vec<_>>().join(",");
        out.row(format!("{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{}\n",
                        &key,
                        max,
                        pc.percentile(&0.95).unwrap().unwrap(),
                        pc.percentile(&0.85).unwrap().unwrap(),
                        pc.percentile(&0.75).unwrap().unwrap(),
                        min,
                        ss.mean().unwrap(),
                        ss.standard_deviation().unwrap_or(0.0),
                        ss.sum(),
                        hist));
    }
}

enum Out {
    Term(TabWriter<std::io::Stderr>, String, &'static [u8]),
    File(File),
}
impl Out {
    fn try_new(file: Option<&String>) -> Result<Self, std::io::Error> {
        match file {
            None => {
                let mut tw = TabWriter::new(stderr()).alignment(tabwriter::Alignment::Right);
                writeln!(tw, "\n\x1B[32mtest\tcmd\tmax\t95%\t85%\t75%\tmin\tmean\tstddev\ttot\tbucketed values")?;
                Ok(Self::Term(tw, String::new(), b""))
            },
            Some(f) => {
                let f =
                    std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(f)?;
                Ok(Self::File(f))
            },
        }
    }
    fn row(&mut self, line: String) {
        match self {
            Self::Term(tw, prev, color) => {
                let cmd = line.split_once("\t").expect("key without a tab").0;
                if prev != cmd {
                    *prev = cmd.into();
                    *color = if *color == b"\x1B[00m" { b"\x1B[96m" } else { b"\x1B[00m" };
                }
                let _ = tw.write(color);
                let _ = tw.write(line.as_bytes());
            },
            Self::File(f) => {
                let _ = f.write_all(line.as_bytes());
            },
        }
    }
}
impl Drop for Out {
    fn drop(&mut self) {
        match self {
            Self::Term(tw, _, _) => tw.flush().unwrap(),
            Self::File(_) => (),
        }
    }
}
