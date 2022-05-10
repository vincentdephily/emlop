#!/usr/bin/env scriptisto

//! Quick script to benchmark *lop implementations.

//! scriptisto-begin
//! script_src: src/main.rs
//! build_cmd: cargo build --release
//! target_bin: ./target/release/exec_compare
//! files:
//!  - path: Cargo.toml
//!    content: |
//!     package = { name = "exec_compare", version = "0.1.0", edition = "2018"}
//!     [dependencies]
//!     clap = "*"
//!     stats-cli = "*"
//!     tabwriter = "*"
//!     rand = "*"
//! scriptisto-end

use clap::{value_t, values_t, App, AppSettings, Arg};
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
        // Minimal "show all merges" command (genlop adds version, emlop adds version+duration)
        ("l", "genlop", &["-f","./benches/emerge.log","-l"], None),
        ("l", "qlop",   &["-f","./benches/emerge.log","-m"], None),
        ("l", "emlop",  &["-F","./benches/emerge.log","l"],  None),
        // Show all merges+unmegres with version and duration
        ("ltmu", "genlop", &["-f","./benches/emerge.log","-lut"],     None),
        ("ltmu", "qlop",   &["-f","./benches/emerge.log","-muUvt"],   None),
        ("ltmu", "emlop",  &["-F","./benches/emerge.log","l","-smu"], None),
        // Show recent sync history
        ("ls", "genlop", &["-f","./benches/emerge.log","-r","-d","1 week ago"],  None),
        ("ls", "qlop",   &["-f","./benches/emerge.log","-st","-d","1 week ago"], None),
        ("ls", "emlop",  &["-F","./benches/emerge.log","l","-ss","-f","1w"],     None),
        // Read only part of a file
        ("ld1", "genlop", &["-f","./benches/emerge.log","-l", "--date","2019-02-01","--date","2019-02-28"], None),
        ("ld1", "qlop",   &["-f","./benches/emerge.log","-mv","--date","2019-02-01","--date","2019-02-28"], None),
        ("ld1", "emlop",  &["-F","./benches/emerge.log","l",  "--from","2019-02-01","--to",  "2019-02-28"], None),
        ("ld2", "genlop", &["-f","./benches/emerge.log","-l", "--date","2020-10-01","--date","2020-10-31"], None),
        ("ld2", "qlop",   &["-f","./benches/emerge.log","-mv","--date","2020-10-01","--date","2020-10-31"], None),
        ("ld2", "emlop",  &["-F","./benches/emerge.log","l",  "--from","2020-10-01","--to",  "2020-10-31"], None),
        // Read a small file
        ("lf", "genlop", &["-f","./benches/emerge.log","-l"], None),
        ("lf", "qlop",   &["-f","./benches/emerge.log","-mv"], None),
        ("lf", "emlop",  &["-F","./benches/emerge.log","l"], None),
        // Force/prevent color output
        ("lc", "emlop",  &["-F","./benches/emerge.log","l","--color=y"],   None),
        ("ln", "genlop", &["-f","./benches/emerge.log","-l","-n"],         None),
        ("ln", "qlop",   &["-f","./benches/emerge.log","-mv","--nocolor"], None),
        ("ln", "emlop",  &["-F","./benches/emerge.log","l","--color=n"],   None),
        // Simple package merge log
        ("egcc", "genlop", &["-f","./benches/emerge.log","-e","gcc"],     None),
        ("egcc", "qlop",   &["-f","./benches/emerge.log","gcc"],          None),
        ("egcc", "emlop",  &["-F","./benches/emerge.log","l","gcc","-e"], None),
        // Version+duration package merge+unmerge log (can't show just package unmerges iin genlop)
        ("tgcc", "genlop", &["-f","./benches/emerge.log","-te","gcc"],          None),
        ("tgcc", "qlop",   &["-f","./benches/emerge.log","-tvmuU","gcc"],        None),
        ("tgcc", "emlop",  &["-F","./benches/emerge.log","l","gcc","-e","-smu"], None),
        // Predict current merge
        ("c", "genlop", &["-c"], None),
        ("c", "qlop",   &["-r"], None),
        ("c", "emlop",  &["p"],  None),
        // Predict merge list
        ("pgcc", "genlop", &["-f","./benches/emerge.log","-p"], Some("benches/emerge-p.gcc.out")),
        ("pgcc", "emlop",  &["-F","./benches/emerge.log","p"],  Some("benches/emerge-p.gcc.out")),
        ("pqt",  "genlop", &["-f","./benches/emerge.log","-p"], Some("benches/emerge-p.qt.out")),
        ("pqt",  "emlop",  &["-F","./benches/emerge.log","p"],  Some("benches/emerge-p.qt.out")),
        ("pkde", "genlop", &["-f","./benches/emerge.log","-p"], Some("benches/emerge-p.kde.out")),
        ("pkde", "emlop",  &["-F","./benches/emerge.log","p"],  Some("benches/emerge-p.kde.out")),
        // Show all info about a specific package
        ("igcc", "genlop", &["-f","./benches/emerge.log","-i","gcc"],     None),
        ("igcc", "qlop",   &["-f","./benches/emerge.log","-c","gcc"],     None),
        ("igcc", "emlop",  &["-F","./benches/emerge.log","s","gcc","-e"], None),
        // Show overall stats
        ("s", "emlop",  &["-F","./benches/emerge.log","s","-sa"], None),
    ];

    // CLI definition
    let mut allprogs: Vec<&str> = defs.iter().map(|&(_, p, _, _)| p).collect();
    allprogs.sort();
    allprogs.dedup();
    let mut allsets: Vec<&str> = defs.iter().map(|&(s, _, _, _)| s).collect();
    allsets.sort();
    allsets.dedup();
    let allsets_str = allsets.join(",");
    let cli = App::new("emlop-bench")
        .about("Quick script to benchmark *lop implementations.")
        .global_setting(AppSettings::ColoredHelp)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .after_help("All benchmarks are biased. Some tips to be less wrong:\n\
 * Make your system is as idle as possible, shutdown unneeded apps (browser, im, cron...).\n\
 * Don't compare numbers collected at different times or on different machines.\n\
 * Look at all indicators, not just the mean.\n\
 * The terminal emulator's speed makes a big difference. Reduce the scroll buffer size and check performance-related settings.\n\
 * Use -n option (redirect to /dev/null) to ignore terminal overhead.\n\
 * Pipe to cat to disable colors (see also color-specific sets).")
        .arg(Arg::with_name("programs")
             .help("Programs to test, formated as 'NAME[:PATH][,...]': coma-separated list, name can \
be abbreviated, alternative path can be provided, eg 'emlop,e:target/release/emlop,q'")
             .short("p")
             .takes_value(true)
             .multiple(true)
             .use_delimiter(true)
             .default_value("emlop"))
        .arg(Arg::with_name("sets")
             .help("Test sets")
             .short("s")
             .takes_value(true)
             .multiple(true)
             .use_delimiter(true)
             .possible_values(&allsets)
             .hide_possible_values(true)
             .default_value(&allsets_str))
        .arg(Arg::with_name("runs")
             .help("Number of iterations")
             .short("r")
             .takes_value(true)
             .default_value("10"))
        .arg(Arg::with_name("bucket")
             .help("Size of histogram buckets")
             .short("b")
             .takes_value(true)
             .default_value("5"))
        .arg(Arg::with_name("nullout")
             .short("n")
             .help("Send test program outputs to /dev/null"))
        .get_matches();

    // CLI parsing
    let runs = value_t!(cli, "runs", usize).unwrap();
    let bucket = value_t!(cli, "bucket", u64).unwrap();
    let progs = values_t!(cli.values_of("programs"), String).unwrap();
    let sets = values_t!(cli.values_of("sets"), String).unwrap();
    let nullout = cli.is_present("nullout");

    // Construct the test list.
    let mut tests = Vec::<(String, &str, &[&str], Option<&str>)>::new();
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
            if &prg == pname && sets.contains(&set.to_string()) {
                found.push(set);
                let cmd = format!("{}\t{} {}{}",
                                  set,
                                  ppath,
                                  args.join(" "),
                                  si.map_or(String::new(), |s| format!(" < {}", s)));
                for _ in 0..runs {
                    tests.push((cmd.clone(), ppath, args, si));
                }
            }
        }
        sets.iter()
            .filter(|s| !found.contains(&s.as_str()))
            .for_each(|s| eprintln!("Test {:?} not defined for {:?}.", s, pname));
    }

    // Load /var/log/emerge.log in the OS cache
    assert_eq!(0,
               Command::new("cat").arg("/var/log/emerge.log")
                                  .stdout(Stdio::null())
                                  .status()
                                  .unwrap()
                                  .code()
                                  .unwrap());

    // Run the tests and collect the results
    tests.shuffle(&mut rand::thread_rng());
    let mut n = tests.len();
    let mut times: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (name, bin, args, stdin) in &tests {
        match nullout {
            true => eprint!("\r{} ", n),
            false => eprintln!(">>>>>> {} {}", n, &name),
        }
        n -= 1;
        let si = stdin.map_or(Stdio::inherit(), |f| File::open(f).unwrap().into());
        let so = if nullout { Stdio::null() } else { Stdio::inherit() };
        let start = Instant::now();
        Command::new(bin).args(args.into_iter())
                         .stdin(si)
                         .stdout(so)
                         .status()
                         .expect(&format!("Couldn't run {} {:?}", bin, args));
        let elapsed = start.elapsed().as_millis() as f64;
        times.entry(name.clone()).or_insert(vec![]).insert(0, elapsed);
    }

    // Output the results
    let mut tw = TabWriter::new(stderr());
    let mut prev = String::new();
    let mut color = "";
    writeln!(tw, "\n\x1B[36mtest\tcmd\tmin\t95%\t85%\t75%\tmean\tmax\tstddev\ttot\tbucketed values")
        .unwrap();
    for (key, vals) in times {
        let ss: SummStats<f64> = vals.iter().cloned().collect();
        let mut pc: Percentiles<f64> = vals.iter().cloned().collect();
        let mut hist: BTreeMap<u64, u64> = BTreeMap::new();
        vals.into_iter()
            .map(|v| (v / bucket as f64).round() as u64 * bucket)
            .for_each(|v| *hist.entry(v).or_insert(0) += 1);
        let hist = hist.iter().map(|(k, v)| format!("{}:{}", k, v)).collect::<Vec<_>>().join(",");
        let cmd = key.split_once("\t").expect("key without a tab").0;
        if prev != cmd {
            color = if color == "\x1B[00m" { "\x1B[37m" } else { "\x1B[00m" };
            prev = cmd.into();
        }
        writeln!(tw,
                 "{}{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{}\t{:.0}\t{:.0}\t{}",
                 color,
                 key,
                 ss.min().unwrap(),
                 pc.percentile(&0.95).unwrap().unwrap(),
                 pc.percentile(&0.85).unwrap().unwrap(),
                 pc.percentile(&0.75).unwrap().unwrap(),
                 ss.mean().unwrap(),
                 ss.max().unwrap(),
                 ss.standard_deviation().unwrap_or(0.0),
                 ss.sum(),
                 hist).unwrap();
    }
    tw.flush().unwrap();
}
