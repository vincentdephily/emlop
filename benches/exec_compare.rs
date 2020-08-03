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
//! scriptisto-end

use clap::{App, AppSettings, Arg, value_t, values_t};
use inc_stats::*;
use std::{collections::{BTreeMap, HashMap}, fs::File, io, io::Write, process::{Command, Stdio}, time::Instant};
use tabwriter::TabWriter;

fn main() {
    // Test definitions: (test suite, program name, program args, stdin)
    let tests: Vec<(&str,&str,&[&str],Option<&str>)> = vec![
        ("h", "genlop", &["-h"], None),
        ("h", "qlop",   &["-h"], None),
        ("h", "emlop",  &["-h"], None),
        ("h", "pqlop",  &["-h"], None),
        ("h", "golop",  &["-h"], None),

        ("l", "genlop", &["-l"], None),
        ("l", "qlop",   &["-mv"], None),
        ("l", "emlop",  &["l"],  None),
        ("l", "golop",  &[],     None),

        ("ld1", "genlop", &["-l","--date","2015-01-01","--date","2015-01-10"], None),
        ("ld1", "qlop",   &["-mv","--date","2015-01-01","--date","2015-01-10"], None),
        ("ld1", "emlop",  &["l","--from","2015-01-01","--to","2015-01-10"],  None),
        ("ld2", "genlop", &["-l","--date","2018-01-01","--date","2018-12-31"], None),
        ("ld2", "qlop",   &["-mv","--date","2018-01-01","--date","2018-12-31"], None),
        ("ld2", "emlop",  &["l","--from","2018-01-01","--to","2018-12-31"],  None),
        ("ld3", "genlop", &["-l","--date","2016-01-01","--date","2018-12-31"], None),
        ("ld3", "qlop",   &["-mv","--date","2016-01-01","--date","2018-12-31"], None),
        ("ld3", "emlop",  &["l","--from","2016-01-01","--to","2018-12-31"],  None),

        ("lf", "genlop", &["-l","-f","test/emerge.10000.log"], None),
        ("lf", "qlop",   &["-mv","-f","test/emerge.10000.log"], None),
        ("lf", "emlop",  &["l", "-F","test/emerge.10000.log"], None),
        ("lf", "golop",  &["-l","test/emerge.10000.log"],      None),

        ("lc", "emlop",  &["l","--color=y"],  None),
        ("ln", "genlop", &["-l","-n"],        None),
        ("ln", "qlop",   &["-mv","--nocolor"], None),
        ("ln", "emlop",  &["l","--color=n"],  None),

        ("tgcc", "genlop", &["-t","gcc"],     None),
        ("tgcc", "qlop",   &["-tv","gcc"],     None),
        ("tgcc", "emlop",  &["l","gcc","-e"], None),
        ("tgcc", "pqlop",  &["-g","gcc"],     None),
        ("tgcc", "golop",  &["-t","gcc"],     None),

        ("egcc", "genlop", &["-e","gcc"],     None),
        ("egcc", "qlop",   &["-mv","gcc"],     None),
        ("egcc", "emlop",  &["l","gcc","-e"], None),
        ("egcc", "pqlop",  &["-g","gcc"],     None),
        ("egcc", "golop",  &["-t","gcc"],     None),

        ("c", "genlop", &["-c"], None),
        ("c", "qlop",   &["-r"], None),
        ("c", "emlop",  &["p"],  None),
        ("c", "pqlop",  &["-c"], None),
        ("c", "golop",  &["-c"], None),

        ("pgcc", "genlop", &["-p"], Some("benches/emerge-p.gcc.out")),
        ("pgcc", "emlop",  &["p"],  Some("benches/emerge-p.gcc.out")),
        ("pqt",  "genlop", &["-p"], Some("benches/emerge-p.qt.out")),
        ("pqt",  "emlop",  &["p"],  Some("benches/emerge-p.qt.out")),
        ("pkde", "genlop", &["-p"], Some("benches/emerge-p.kde.out")),
        ("pkde", "emlop",  &["p"],  Some("benches/emerge-p.kde.out")),

        ("i", "genlop", &["-i","gcc"], None),
        ("i", "qlop",   &["-c","gcc"], None),
        ("i", "emlop",  &["s","gcc","-e"],  None),
        ("i", "pqlop",  &["-g","gcc"], None),
        ("i", "golop",  &["-t","gcc"], None),
    ];

    // CLI definition
    let mut allprogs: Vec<&str> = tests.iter().map(|&(_,p,_,_)| p).collect();
    allprogs.sort();
    allprogs.dedup();
    let mut allsuites: Vec<&str> = tests.iter().map(|&(s,_,_,_)| s).collect();
    allsuites.sort();
    allsuites.dedup();
    let allsuites_str = allsuites.join(",");
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
 * Pipe to cat to disable colors (see also color-specific suites).")
        .arg(Arg::with_name("programs")
             .help("Programs to test, formated as 'NAME[:PATH][,...]': coma-separated list, name can \
be abbreviated, alternative path can be provided, eg 'emlop,e:target/release/emlop,q'")
             .short("p")
             .takes_value(true)
             .multiple(true)
             .use_delimiter(true)
             .default_value("emlop"))
        .arg(Arg::with_name("suites")
             .help("Test suites")
             .short("s")
             .takes_value(true)
             .multiple(true)
             .use_delimiter(true)
             .possible_values(&allsuites)
             .hide_possible_values(true)
             .default_value(&allsuites_str))
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
    let suites = values_t!(cli.values_of("suites"), String).unwrap();
    let nullout = cli.is_present("nullout");

    // Construct the test list. We abuse the hashmap behavior to run tests in random order.
    let mut mytests = HashMap::<usize,(String,&str,&[&str],Option<&str>)>::new();
    let mut n = 0;
    for p in progs.iter() {
        let (mut p1,mut p2) = p.split_at(p.find(':').unwrap_or(p.len()));
        let pmatch: Vec<&str> = allprogs.clone().into_iter().filter(|s| s.starts_with(p1)).collect();
        if 1 != pmatch.len() {
            writeln!(io::stderr(), "Found {} match for {:?}, should match exactly one of {}", pmatch.len(), p1, allprogs.join(",")).unwrap();
            ::std::process::exit(1);
        }
        p1 = pmatch[0];
        p2 = match p2.is_empty() {
            true => p1,
            false => p2.trim_left_matches(':'),
        };
        let tests: Vec<_> = tests.iter().filter(|&(s,t,_,_)| t == &p1 && suites.contains(&s.to_string())).collect();
        let foundsuites: Vec<String> = tests.iter().map(|&(su,_,_,_)| su.to_string()).collect();
        suites.iter().filter(|s| !foundsuites.contains(s)).for_each(|s| writeln!(io::stderr(), "Test suite {} not defined for {}.", s, p1).unwrap());
        for &(su,_,ar,si) in tests {
            for _ in 0..runs {
                mytests.insert(n,(format!("{}\t{}",su,p2), p2, ar, si));
                n += 1;
            }
        }
    }

    // Load /var/log/emerge.log in the OS cache
    assert_eq!(0, Command::new("cat")
               .arg("/var/log/emerge.log")
               .stdout(Stdio::null())
               .status().unwrap().code().unwrap());

    // Run the tests and collect the results
    let mut times: BTreeMap<String,Vec<f64>> = BTreeMap::new();
    for (name,bin,args,stdin) in mytests.values() {
        match nullout {
            true => write!(io::stderr(), "\r{} ", n).unwrap(),
            false => writeln!(io::stderr(), "{}: {} {}{}", n, bin, args.join(" "), stdin.map_or(String::new(), |f| format!(" < {}", f))).unwrap(),
        };
        n -= 1;
        let timevec = times.entry(name.clone()).or_insert(vec![]);
        let si = match stdin {
            None => Stdio::inherit(),
            Some(f) => File::open(f).unwrap().into()
        };
        let so = match nullout {
            true => Stdio::null(),
            false => Stdio::inherit(),
        };
        let start = Instant::now();
        Command::new(bin)
            .args(args.into_iter())
            .stdin(si)
            .stdout(so)
            .status()
            .expect(&format!("Couldn't run {} {:?}", bin, args));
        let elapsed = start.elapsed();
        timevec.insert(0, (elapsed.as_secs()*1000 + elapsed.subsec_nanos() as u64 / 1_000_000) as f64);
    }

    // Output the results
    let mut tw = TabWriter::new(io::stderr());
    writeln!(tw,"\nsuite\tprog\tmin\t95%\t85%\t75%\tmean\tmax\tstddev\ttot\tvalues").unwrap();
    for (key,vals) in times {
        let ss: SummStats = vals.iter().cloned().collect();
        let mut pc: Percentiles = vals.iter().cloned().collect();
        let mut hist: BTreeMap<u64,u64> = BTreeMap::new();
        vals.into_iter().for_each(|v| *hist.entry((v/bucket as f64).round() as u64 * bucket).or_insert(0) += 1);
        writeln!(tw, "{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{}\t{:.0}\t{:.0}\t{}", key,
                 ss.min().unwrap(),
                 pc.percentile(&0.95).unwrap(),
                 pc.percentile(&0.85).unwrap(),
                 pc.percentile(&0.75).unwrap(),
                 ss.mean().unwrap(),
                 ss.max().unwrap(),
                 ss.standard_deviation().unwrap_or(0.0),
                 ss.sum(),
                 hist.iter().map(|(k,v)| format!("{}:{}",k,v)).collect::<Vec<String>>().join(","),
        ).unwrap();
    }
    tw.flush().unwrap();
}
