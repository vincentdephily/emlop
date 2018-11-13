use std::collections::{BTreeMap, HashMap};
use std::io::{stdin, stdout, Stdout};

use crate::*;
use crate::parser::*;
use crate::proces::*;

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_list(args: &ArgMatches, subargs: &ArgMatches, st: Styles) -> Result<bool, Error> {
    let show = subargs.value_of("show").unwrap();
    let show_merge = show.contains(&"m") || show.contains(&"a");
    let show_sync = show.contains(&"s") || show.contains(&"a");
    let hist = parser::new_hist(myopen(args.value_of("logfile").unwrap())?, args.value_of("logfile").unwrap().into(),
                                value_opt(args, "from", parse_date), value_opt(args, "to", parse_date),
                                show_merge, show_sync,
                                subargs.value_of("package"), subargs.is_present("exact"))?;
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut found_one = false;
    let mut syncstart: i64 = 0;
    for p in hist {
        match p {
            ParsedHist::Start{ts, ebuild, version, iter, ..} => {
                // This'll overwrite any previous entry, if a build started but never finished
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            ParsedHist::Stop{ts, ebuild, version, iter, ..} => {
                found_one = true;
                let started = started.remove(&(ebuild.clone(), version.clone(), iter.clone()));
                writeln!(stdout(), "{} {}{:>9} {}{}-{}{}",
                         fmt_time(ts), st.dur_p, started.map_or(String::from("?"), |pt| fmt_duration(ts-pt)),
                         st.pkg_p, ebuild, version, st.pkg_s).unwrap_or(());
            },
            ParsedHist::SyncStart{ts} => {
                syncstart = ts;
            },
            ParsedHist::SyncStop{ts} => {
                found_one = true;
                writeln!(stdout(), "{} {}{:>9}{} Sync", fmt_time(ts), st.dur_p, fmt_duration(ts-syncstart), st.dur_s).unwrap_or(());
            },
        }
    }
    Ok(found_one)
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
pub fn cmd_stats(tw: &mut TabWriter<Stdout>, args: &ArgMatches, subargs: &ArgMatches, st: Styles) -> Result<bool, Error> {
    let show = subargs.value_of("show").unwrap();
    let show_merge = show.contains(&"m") || show.contains(&"a");
    let show_tot = show.contains(&"t") || show.contains(&"a");
    let show_sync = show.contains(&"s") || show.contains(&"a");
    let hist = parser::new_hist(myopen(args.value_of("logfile").unwrap())?, args.value_of("logfile").unwrap().into(),
                                value_opt(args, "from", parse_date), value_opt(args, "to", parse_date),
                                show_merge || show_tot, show_sync,
                                subargs.value_of("package"), subargs.is_present("exact"))?;
    let lim = value(subargs, "limit", parse_limit);
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut syncs: Vec<i64> = vec![];
    let mut times: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut syncstart: i64 = 0;
    for p in hist {
        match p {
            ParsedHist::Start{ts, ebuild, version, iter, ..} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            ParsedHist::Stop{ts, ebuild, version, iter, ..} => {
                if let Some(start_ts) = started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    let timevec = times.entry(ebuild.clone()).or_insert_with(|| vec![]);
                    timevec.insert(0, ts-start_ts);
                }
            },
            ParsedHist::SyncStart{ts} => {
                syncstart = ts;
            },
            ParsedHist::SyncStop{ts} => {
                syncs.push(ts-syncstart);
            },
        }
    };
    let found_one = !times.is_empty() || !syncs.is_empty();
    if show_merge {
        for (pkg,tv) in &times {
            let (predtime,predcount,tottime,totcount) = tv.iter()
                .fold((0,0,0,0), |(pt,pc,tt,tc), &i| {
                    if tc >= lim {(pt,  pc,  tt+i,tc+1)}
                    else         {(pt+i,pc+1,tt+i,tc+1)}
                });
            writeln!(tw, "{}{}\t{}{:>10}\t{}{:>5}\t{}{:>8}{}",
                     st.pkg_p, pkg,
                     st.dur_p, fmt_duration(tottime),
                     st.cnt_p, totcount,
                     st.dur_p, fmt_duration(predtime/predcount), st.dur_s)?;
        }
    }
    if show_tot {
        let mut tottime = 0;
        let mut totcount = 0;
        for tv in times.values() {
            for t in tv {
                tottime += t;
                totcount += 1
            }
        }
        let totavg = if totcount > 0 {tottime/totcount} else {0};
        writeln!(tw, "{}Merge\t{}{:>10}\t{}{:>5}\t{}{:>8}{}",
                 st.pkg_p,
                 st.dur_p, fmt_duration(tottime),
                 st.cnt_p, totcount,
                 st.dur_p, fmt_duration(totavg), st.dur_s)?;
    }
    if show_sync {
        let synctime = syncs.iter().fold(0,|a,t|t+a);
        let synccount = syncs.len() as i64;
        let syncavg = if synccount > 0 {synctime/synccount} else {0};
        writeln!(tw, "{}Sync\t{}{:>10}\t{}{:>5}\t{}{:>8}{}",
                 st.pkg_p,
                 st.dur_p, fmt_duration(synctime),
                 st.cnt_p, synccount,
                 st.dur_p, fmt_duration(syncavg), st.dur_s)?;
    }
    Ok(found_one)
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(tw: &mut TabWriter<Stdout>, args: &ArgMatches, subargs: &ArgMatches, st: Styles) -> Result<bool, Error> {
    let now = epoch_now();
    let lim = value(subargs, "limit", parse_limit);

    // Gather and print info about current merge process.
    let mut cms = std::i64::MAX;
    for i in get_all_info(Some("emerge"))? {
        cms = std::cmp::min(cms, i.start);
        writeln!(tw, "Pid {}: ...{}\t{}{:>9}{}", i.pid, &i.cmdline[(i.cmdline.len()-35)..], st.dur_p, fmt_duration(now-i.start), st.dur_s)?;
    }
    if cms == std::i64::MAX && atty::is(atty::Stream::Stdin) {
        writeln!(tw, "No ongoing merge found")?;
        return Ok(false)
    }

    // Parse emerge log.
    let hist = parser::new_hist(myopen(args.value_of("logfile").unwrap())?, args.value_of("logfile").unwrap().into(),
                                value_opt(args, "from", parse_date), value_opt(args, "to", parse_date),
                                true, false,
                                None, false)?;
    let mut started: HashMap<(String, String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    for p in hist {
        match p {
            // We're ignoring iter here (reducing the start->stop matching accuracy) because there's no iter in the pretend output.
            ParsedHist::Start{ts, ebuild, version, ..} => {
                started.insert((ebuild.clone(), version.clone()), ts);
            },
            ParsedHist::Stop{ts, ebuild, version, ..} => {
                if let Some(start_ts) = started.remove(&(ebuild.clone(), version.clone())) {
                    let timevec = times.entry(ebuild.clone()).or_insert_with(|| vec![]);
                    timevec.insert(0, ts-start_ts);
                }
            },
            ParsedHist::SyncStart{..} => (),
            ParsedHist::SyncStop{..} => (),
        }
    }

    // Parse list of pending merges (from stdin or from emerge log filtered by cms).
    // We collect immediately to deal with type mismatches; it should be a small list anyway.
    let pretend: Vec<ParsedPretend> = if atty::is(atty::Stream::Stdin) {
        started.iter()
            .filter(|&(_,t)| *t > cms)
            .map(|(&(ref e,ref v),_)| ParsedPretend{ebuild:e.to_string(), version:v.to_string()})
            .collect()
    } else {
        parser::new_pretend(stdin(), "STDIN")
    };

    // Gather and print per-package and indivudual stats.
    let mut totcount = 0;
    let mut totunknown = 0;
    let mut totpredict = 0;
    let mut totelapsed = 0;
    for ParsedPretend{ebuild, version} in pretend {
        // Find the elapsed time, if any (heuristic is that emerge process started before
        // this merge finished, it's not failsafe but IMHO no worse than genlop).
        let (elapsed,elapsed_fmt) = match started.remove(&(ebuild.clone(), version.clone())) {
            Some(s) if s > cms => (now - s, format!(" - {}{}{}", st.dur_p, fmt_duration(now-s), st.dur_s)),
            _ => (0, "".into())
        };

        // Find the predicted time and adjust counters
        totcount += 1;
        let pred_fmt = match times.get(&ebuild) {
            Some(tv) => {
                let (predtime,predcount,_) = tv.iter()
                    .fold((0,0,0), |(pt,pc,tc), &i| {
                        if tc >= lim {(pt,  pc,  tc+1)}
                        else         {(pt+i,pc+1,tc+1)}
                    });
                totpredict += predtime / predcount;
                if elapsed > 0 {
                    totelapsed += elapsed;
                    totpredict -= std::cmp::min(predtime / predcount, elapsed);
                }
                fmt_duration(predtime/predcount)
            },
            None => {
                totunknown += 1;
                "?".into()
            },
        };

        // Done
        writeln!(tw, "{}{}-{}\t{}{:>9}{}{}", st.pkg_p, ebuild, version, st.dur_p, pred_fmt, st.dur_s, elapsed_fmt)?;
    }
    if totcount > 0 {
        writeln!(tw, "Estimate for {}{}{} ebuilds ({}{}{} unknown, {}{}{} elapsed)\t{}{:>9}{}",
                 st.cnt_p, totcount, st.cnt_s,
                 st.cnt_p, totunknown, st.cnt_s,
                 st.dur_p, fmt_duration(totelapsed), st.dur_s,
                 st.dur_p, fmt_duration(totpredict), st.dur_s)?;
    } else {
        writeln!(tw, "No pretended merge found")?;
    }
    Ok(totcount > 0)
}


#[cfg(test)]
mod tests {
    use assert_cli::Assert;
    use indoc::*;
    //TODO: Simplify fails_with() calls once https://github.com/assert-rs/assert_cli/issues/99 is closed

    #[test]
    fn log() {
        let t: Vec<(&[&str],&str,i32)> = vec![
            // Basic test
            (&["-f","test/emerge.10000.log","l","client"],
             indoc!("2018-02-04 04:55:19 +00:00     35:46 mail-client/thunderbird-52.6.0\n\
                     2018-02-04 05:42:48 +00:00     47:29 www-client/firefox-58.0.1\n\
                     2018-02-09 11:04:59 +00:00     47:58 mail-client/thunderbird-52.6.0-r1\n\
                     2018-02-12 10:14:11 +00:00        31 kde-frameworks/kxmlrpcclient-5.43.0\n\
                     2018-02-16 04:41:39 +00:00   6:03:14 www-client/chromium-64.0.3282.140\n\
                     2018-02-19 17:35:41 +00:00   7:56:03 www-client/chromium-64.0.3282.167\n\
                     2018-02-22 13:32:53 +00:00        44 www-client/links-2.14-r1\n\
                     2018-02-28 09:14:37 +00:00      6:02 www-client/falkon-3.0.0\n\
                     2018-03-06 04:19:52 +00:00   7:42:07 www-client/chromium-64.0.3282.186\n\
                     2018-03-12 10:35:22 +00:00        14 x11-apps/xlsclients-1.1.4\n\
                     2018-03-12 11:03:53 +00:00        16 kde-frameworks/kxmlrpcclient-5.44.0\n"),
             0),
            // Check output when duration isn't known
            (&["-f","test/emerge.10000.log","l","-s","m","mlt","-e","--from","2018-02-18 12:37:00"],
             indoc!("2018-02-18 12:37:09 +00:00         ? media-libs/mlt-6.4.1-r6\n\
                     2018-02-27 15:10:05 +00:00        43 media-libs/mlt-6.4.1-r6\n\
                     2018-02-27 16:48:40 +00:00        39 media-libs/mlt-6.4.1-r6\n"),
             0),
            // Check output of sync events
            (&["-f","test/emerge.10000.log","l","-ss","--from","2018-03-07 10:42:00","--to","2018-03-07 14:00:00"],
             indoc!("2018-03-07 11:37:05 +00:00        38 Sync\n\
                     2018-03-07 13:56:09 +00:00        40 Sync\n"),
             0),
            (&["-f","test/emerge.10000.log","l","--show","ms","--from","2018-03-07 10:42:00","--to","2018-03-07 14:00:00"],
             indoc!("2018-03-07 10:43:10 +00:00        14 sys-apps/the_silver_searcher-2.0.0\n\
                     2018-03-07 11:37:05 +00:00        38 Sync\n\
                     2018-03-07 12:49:13 +00:00      1:01 sys-apps/util-linux-2.30.2-r1\n\
                     2018-03-07 13:56:09 +00:00        40 Sync\n\
                     2018-03-07 13:59:41 +00:00        24 dev-libs/nspr-4.18\n"),
             0),
        ];
        for (a,o,e) in t {
            match e {
                0 => Assert::main_binary().with_args(a).stdout().is(o).unwrap(),
                _ => Assert::main_binary().with_args(a).fails_with(e).stdout().is(o).unwrap(),
            }
        }
    }

    #[test]
    fn predict_tty() {
        // This depends on there being no currently running emerge.
        // Not a hugely useful test, but it's something.
        let o = "No pretended merge found\n";
        Assert::main_binary().with_args(&["p"]).fails_with(2).stdout().is(o).unwrap();
    }

    #[test]
    fn predict_emerge_p() {
        let t = vec![
            // Check garbage input
            (indoc!("blah blah\n"),
             indoc!("No pretended merge found\n"),
             2),
            // Check all-unknowns
            (indoc!("[ebuild   R   ~] dev-lang/unknown-1.42\n"),
             indoc!("dev-lang/unknown-1.42                                  ?\n\
                     Estimate for 1 ebuilds (1 unknown, 0 elapsed)          0\n"),
             0),
            // Check that unknown ebuild don't wreck alignment. Remember that times are {:>9}
            (indoc!("[ebuild   R   ~] dev-qt/qtcore-5.9.4-r2\n\
                     [ebuild   R   ~] dev-lang/unknown-1.42\n\
                     [ebuild   R   ~] dev-qt/qtgui-5.9.4-r3\n"),
             indoc!("dev-qt/qtcore-5.9.4-r2                              3:44\n\
                     dev-lang/unknown-1.42                                  ?\n\
                     dev-qt/qtgui-5.9.4-r3                               4:36\n\
                     Estimate for 3 ebuilds (1 unknown, 0 elapsed)       8:20\n"),
             0),
        ];
        for (i,o,e) in t {
            match e {
                0 => Assert::main_binary().with_args(&["-f","test/emerge.10000.log","p"])
                    .stdin(i).stdout().is(o).unwrap(),
                _ => Assert::main_binary().with_args(&["-f","test/emerge.10000.log","p"])
                    .fails_with(e).stdin(i).stdout().is(o).unwrap(),
            }
        }
    }

    #[test]
    fn stats() {
        let t: Vec<(&[&str],&str,i32)> = vec![
            (&["-f","test/emerge.10000.log","s","client"],
             indoc!("kde-frameworks/kxmlrpcclient          47      2        23\n\
                     mail-client/thunderbird          1:23:44      2     41:52\n\
                     www-client/chromium             21:41:24      3   7:13:48\n\
                     www-client/falkon                   6:02      1      6:02\n\
                     www-client/firefox                 47:29      1     47:29\n\
                     www-client/links                      44      1        44\n\
                     x11-apps/xlsclients                   14      1        14\n"),
             0),
            (&["-f","test/emerge.10000.log","s","client","-ss"],
             indoc!("Sync     1:19:28    150        31\n"),
             0),
            (&["-f","test/emerge.10000.log","s","client","-sst"],
             indoc!("Merge    24:00:24     11   2:10:56\n\
                     Sync      1:19:28    150        31\n"),
             0),
            (&["-f","test/emerge.10000.log","s","client","-sa"],
             indoc!("kde-frameworks/kxmlrpcclient          47      2        23\n\
                     mail-client/thunderbird          1:23:44      2     41:52\n\
                     www-client/chromium             21:41:24      3   7:13:48\n\
                     www-client/falkon                   6:02      1      6:02\n\
                     www-client/firefox                 47:29      1     47:29\n\
                     www-client/links                      44      1        44\n\
                     x11-apps/xlsclients                   14      1        14\n\
                     Merge                           24:00:24     11   2:10:56\n\
                     Sync                             1:19:28    150        31\n"),
             0),
            (&["-f","test/emerge.10000.log","s","--from","2018-02-03T23:11:47","--to","2018-02-04","notfound","-sa"],
             indoc!("Merge           0      0         0\n\
                     Sync            0      0         0\n"),
             2),
        ];
        for (a,o,e) in t {
            match e {
                0 => Assert::main_binary().with_args(a).stdout().is(o).unwrap(),
                _ => Assert::main_binary().with_args(a).fails_with(e).stdout().is(o).unwrap(),
            }
        }
    }

    #[test]
    fn exit_status() {
        // 0: no problem
        // 1: user or program error
        // 2: command ran properly but didn't find anything
        let t: Vec<(&[&str],i32)> = vec![
            // Help, version, badarg (clap)
            (&["-h"], 0),
            (&["-V"], 0),
            (&["l","-h"], 0),
            (&[], 1),
            (&["s","--foo"], 1),
            (&["badcmd"], 1),
            // Bad arguments (emlop)
            (&["l","--logfile","notfound"], 1),
            (&["s","--logfile","notfound"], 1),
            (&["p","--logfile","notfound"], 1),
            (&["l","bad regex [a-z"], 1),
            (&["s","bad regex [a-z"], 1),
            (&["p","bad regex [a-z"], 1),
            // Normal behaviour
            (&["-f","test/emerge.10000.log","p"], 2),
            (&["-f","test/emerge.10000.log","l"], 0),
            (&["-f","test/emerge.10000.log","l","-s"], 0),
            (&["-f","test/emerge.10000.log","l","-e","icu"], 0),
            (&["-f","test/emerge.10000.log","l","-e","unknown"], 2),
            (&["-f","test/emerge.10000.log","l","--from","2018-09-28"], 2),
            (&["-f","test/emerge.10000.log","l","-s","--from","2018-09-28"], 2),
            (&["-f","test/emerge.10000.log","s"], 0),
            (&["-f","test/emerge.10000.log","s","-e","icu"], 0),
            (&["-f","test/emerge.10000.log","s","-e","unknown"], 2),
        ];
        for (args, exit) in t {
            match exit {
                0 => Assert::main_binary().with_args(args).unwrap(),
                _ => Assert::main_binary().with_args(args).fails_with(exit).unwrap(),
            }
        }
    }
}
