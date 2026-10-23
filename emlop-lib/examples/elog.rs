use emlop_lib::{ArgParse, DEFAULT_EMERGELOG, ELog, ELogLine, FmtUtc, HistBound};
use std::{collections::HashMap,
          env::{args, var}};
use time::UtcOffset;

fn main() {
    let mut args: Vec<_> = args().collect();
    if args.len() > 1 {
        let prog = args.remove(0);
        eprintln!("This example demonstrates the basic of ELog parsing.\n\n\
                   Usage: <VAR=val, ...> {prog}\n \
                   * FILE: emerge.log file, eg ./testdata/emerge.10000.log\n \
                   * SEARCH: search regex\n \
                   * WHAT: what to parse, eg m/u/s/r\n \
                   * FROM: when to parse from\n \
                   * TO: when to parse up to\n \
                   * MAX: how many events to display\n \
                   * RUST_LOG: log level, eg info/debug/trace\n");
    } else if let Err(e) = elog() {
        eprintln!("Error: {e}");
    }
}

fn elog() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    let _ = env_logger::init();

    // Parse user input. Emlop-lib provides some default values as constants, and some parser using
    // the ArgParse trait
    let file = var("FILE").unwrap_or(DEFAULT_EMERGELOG.into());
    let search = var("SEARCH").map(|term| vec![term]).unwrap_or_default();
    let what = var("WHAT").as_deref().unwrap_or("m").to_string();
    let offset = UtcOffset::current_local_offset()?;
    let start = var("FROM").map_or(Ok(HistBound::None), |s| HistBound::parse(&s, offset, "FROM"))?;
    let stop = var("TO").map_or(Ok(HistBound::None), |s| HistBound::parse(&s, offset, "TO"))?;
    let max = var("MAX").as_deref().unwrap_or("20").parse::<usize>()?;

    // Create the struct that holds `emerge.log` parsing options. File is opened immediately, and
    // can be gzip-compressed
    let elog = ELog::new(&file)?
                                // Set a search term to filter ebuilds or sync repos. Multiple terms
                                // can be provided, the boolean specifies full string match instead
                                // of regex match.
                                .search(&search, false)?
                                // Set the min/max timestamps to read from the file. An "nth
                                // command" bound will do an optimized parse to find the command
                                // timestamps
                                .bounds(start, stop)?
                                // Set what types of ELogLines we want to get, defaults to 'merge'
                                .merge(what.contains('m'))
                                .unmerge(what.contains('u'))
                                .sync(what.contains('s'))
                                .run(what.contains('r'))
                                // Now that everything is configured, start parsing and return an
                                // interator over ELogLine
                                .get();

    // Init some state to use while looping through ELogLines
    let mut merges: HashMap<String, (i64, &'static str)> = HashMap::new();
    let mut prev_sync = None;
    let mut counts: HashMap<String, usize> = HashMap::new();

    // Each parsed/matched log line
    for l in elog {
        // Gather stats on number of ElogLine variants
        let kind = format!("{l:?}").split_once(' ').unwrap().0.to_string();
        counts.entry(kind).and_modify(|c| *c += 1).or_insert(1);
        match l {
            // Handle merge event: save the start time by `key`, and assume it's a normal compilation merge
            ELogLine::MergeStart { key, ts, .. } => {
                merges.insert(key, (ts, "BUILD"));
            },
            // A separate elog tells us if it is a binary merge
            ELogLine::MergeBin { key, .. } => {
                if let Some((_, op)) = merges.get_mut(&key) {
                    *op = "BIN";
                }
            },
            // Now that we have the stop time, retrieve the start time and build/bin flag from the hash
            ELogLine::MergeStop { ref key, ts, .. } => {
                let (started, op) = merges.remove(key).unwrap_or((ts + 1, "LOST"));
                if counts.values().sum::<usize>() <= max {
                    println!("{} {op} {} in {}s", FmtUtc(ts), l.ebuild_version(), ts - started);
                }
            },
            // If you don't care about duration you can simplify things
            ELogLine::UnmergeStart { .. } => (),
            ELogLine::UnmergeStop { ts, .. } => {
                if counts.values().sum::<usize>() <= max {
                    println!("{} CLEAN {}", FmtUtc(ts), l.ebuild_version());
                }
            },
            // For sync events there's no `key` to match, so just assume the stop is for the previous start
            ELogLine::SyncStart { ts, .. } => prev_sync = Some(ts),
            ELogLine::SyncStop { ref repo, ts, .. } => {
                if counts.values().sum::<usize>() <= max {
                    println!("{} SYNC {} in {}s", FmtUtc(ts), repo, ts - prev_sync.unwrap_or(ts));
                }
            },
            // A catch-all is often necessary, for example we're not handling RunStart in this
            // example, try running the example with WHAT=r
            _ => {
                if counts.values().sum::<usize>() <= max {
                    println!("Unhandled ElogLine::{l:?}");
                }
            },
        }
    }
    println!("Received {counts:?} events from {file} matching {search:?}");
    Ok(())
}
