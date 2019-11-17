use crate::{parser::*, proces::*, *};
use chrono::{Datelike, Duration, Timelike, Weekday};
use std::{collections::{BTreeMap, HashMap},
          io::{stdin, stdout, Stdout}};

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_list(args: &ArgMatches, subargs: &ArgMatches, st: &Styles) -> Result<bool, Error> {
    let show = subargs.value_of("show").unwrap();
    let show_merge = show.contains(&"m") || show.contains(&"a");
    let show_sync = show.contains(&"s") || show.contains(&"a");
    let hist = new_hist(myopen(args.value_of("logfile").unwrap())?,
                        args.value_of("logfile").unwrap().into(),
                        value_opt(args, "from", parse_date),
                        value_opt(args, "to", parse_date),
                        show_merge,
                        show_sync,
                        subargs.value_of("package"),
                        subargs.is_present("exact"))?;
    let fmtd = value_t!(subargs, "duration", DurationStyle).unwrap();
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut found_one = false;
    let mut syncstart: i64 = 0;
    for p in hist {
        match p {
            ParsedHist::Start { ts, ebuild, version, iter, .. } => {
                // This'll overwrite any previous entry, if a build started but never finished
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            ParsedHist::Stop { ts, ebuild, version, iter, .. } => {
                found_one = true;
                let started = started.remove(&(ebuild.clone(), version.clone(), iter.clone()));
                #[rustfmt::skip]
                writeln!(stdout(), "{} {}{:>9} {}{}-{}{}",
                         fmt_time(ts),
                         st.dur_p, fmt_duration(&fmtd, ts-started.unwrap_or(ts+1)),
                         st.pkg_p, ebuild, version, st.pkg_s).unwrap_or(());
            },
            ParsedHist::SyncStart { ts } => {
                syncstart = ts;
            },
            ParsedHist::SyncStop { ts } => {
                found_one = true;
                #[rustfmt::skip]
                writeln!(stdout(), "{} {}{:>9}{} Sync",
                         fmt_time(ts),
                         st.dur_p, fmt_duration(&fmtd, ts-syncstart), st.dur_s).unwrap_or(());
            },
        }
    }
    Ok(found_one)
}

/// Given a unix timestamp, truncate it to midnight and advance by the given number of years/months/days.
/// We avoid DST issues by switching to 12:00.
/// See https://github.com/chronotope/chrono/issues/290
fn timespan_next(ts: i64, add: &Timespan) -> i64 {
    let mut d = Local.timestamp(ts, 0).with_minute(0).unwrap().with_second(0).unwrap();
    match add {
        Timespan::Year => {
            d = d.with_day(1).unwrap().with_month(1).unwrap().with_year(d.year() + 1).unwrap()
        },
        Timespan::Month => {
            d = d.with_day(1)
                 .unwrap()
                 .with_month0((d.month0() + 1) % 12)
                 .unwrap()
                 .with_year(if d.month() == 12 { d.year() + 1 } else { d.year() })
                 .unwrap()
        },
        Timespan::Week => {
            let till_monday = match d.weekday() {
                Weekday::Mon => 7,
                Weekday::Tue => 6,
                Weekday::Wed => 5,
                Weekday::Thu => 4,
                Weekday::Fri => 3,
                Weekday::Sat => 2,
                Weekday::Sun => 1,
            };
            d = d.with_hour(12).unwrap() + Duration::days(till_monday)
        },
        Timespan::Day => d = d.with_hour(12).unwrap() + Duration::days(1),
    }
    let res = d.with_hour(0).unwrap().timestamp();
    debug!("{} + {:?} = {}", fmt_time(ts), add, fmt_time(res));
    res
}
fn timespan_header(ts: i64, timespan: &Timespan) -> String {
    match timespan {
        Timespan::Year => format!("{}", Local.timestamp(ts, 0).format("%Y ")),
        Timespan::Month => format!("{}", Local.timestamp(ts, 0).format("%Y-%m ")),
        Timespan::Week => format!("{}", Local.timestamp(ts, 0).format("%Y-%W ")),
        Timespan::Day => format!("{}", Local.timestamp(ts, 0).format("%Y-%m-%d ")),
    }
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
pub fn cmd_stats(tw: &mut TabWriter<Stdout>,
                 args: &ArgMatches,
                 subargs: &ArgMatches,
                 st: &Styles)
                 -> Result<bool, Error> {
    let show = subargs.value_of("show").unwrap();
    let show_merge = show.contains(&"m") || show.contains(&"a");
    let show_tot = show.contains(&"t") || show.contains(&"a");
    let show_sync = show.contains(&"s") || show.contains(&"a");
    let timespan_opt = value_opt(subargs, "group", parse_timespan);
    let hist = new_hist(myopen(args.value_of("logfile").unwrap())?,
                        args.value_of("logfile").unwrap().into(),
                        value_opt(args, "from", parse_date),
                        value_opt(args, "to", parse_date),
                        show_merge || show_tot,
                        show_sync,
                        subargs.value_of("package"),
                        subargs.is_present("exact"))?;
    let fmtd = value_t!(subargs, "duration", DurationStyle).unwrap();
    let lim = value(subargs, "limit", parse_limit);
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut syncs: Vec<i64> = vec![];
    let mut times: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut syncstart: i64 = 0;
    let mut nextts = 0;
    let mut curts = 0;
    for p in hist {
        if let Some(ref timespan) = timespan_opt {
            let t = p.ts();
            if nextts == 0 {
                nextts = timespan_next(t, timespan);
                curts = t;
            } else if t > nextts {
                let group_by = timespan_header(curts, timespan);
                cmd_stats_group(tw, &st, &fmtd, false, lim, show_merge, show_tot, show_sync,
                                &group_by, &syncs, &times)?;
                syncs.clear();
                times.clear();
                nextts = timespan_next(t, timespan);
                curts = t;
            }
        }
        match p {
            ParsedHist::Start { ts, ebuild, version, iter, .. } => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            ParsedHist::Stop { ts, ebuild, version, iter, .. } => {
                if let Some(start_ts) =
                    started.remove(&(ebuild.clone(), version.clone(), iter.clone()))
                {
                    let timevec = times.entry(ebuild.clone()).or_insert_with(|| vec![]);
                    timevec.insert(0, ts - start_ts);
                }
            },
            ParsedHist::SyncStart { ts } => {
                syncstart = ts;
            },
            ParsedHist::SyncStop { ts } => {
                syncs.push(ts - syncstart);
            },
        }
    }
    let group_by = timespan_opt.map_or(String::new(), |t| timespan_header(curts, &t));
    cmd_stats_group(tw, &st, &fmtd, true, lim, show_merge, show_tot, show_sync, &group_by,
                    &syncs, &times)?;
    Ok(!times.is_empty() || !syncs.is_empty())
}

fn cmd_stats_group(tw: &mut TabWriter<Stdout>,
                   st: &Styles,
                   fmtd: &DurationStyle,
                   print_zeros: bool,
                   lim: u16,
                   show_merge: bool,
                   show_tot: bool,
                   show_sync: bool,
                   group_by: &str,
                   syncs: &[i64],
                   times: &BTreeMap<String, Vec<i64>>)
                   -> Result<(), Error> {
    if show_merge && (print_zeros || !times.is_empty()) {
        for (pkg, tv) in times {
            let (predtime, predcount, tottime, totcount) =
                tv.iter().fold((0, 0, 0, 0), |(pt, pc, tt, tc), &i| {
                             if i < 0 {
                                 (pt, pc, tt, tc + 1)
                             } else if tc >= lim {
                                 (pt, pc, tt + i, tc + 1)
                             } else {
                                 (pt + i, pc + 1, tt + i, tc + 1)
                             }
                         });
            #[rustfmt::skip]
            writeln!(tw, "{}{}{}\t{}{:>10}\t{}{:>5}\t{}{:>8}{}",
                     group_by,
                     st.pkg_p, pkg,
                     st.dur_p, fmt_duration(&fmtd, tottime),
                     st.cnt_p, totcount,
                     st.dur_p, fmt_duration(&fmtd, predtime/predcount), st.dur_s)?;
        }
    }
    if show_tot && (print_zeros || !times.is_empty()) {
        let mut tottime = 0;
        let mut totcount = 0;
        for tv in times.values() {
            for t in tv {
                if *t > 0 {
                    tottime += t;
                }
                totcount += 1
            }
        }
        let totavg = if totcount > 0 { tottime / totcount } else { 0 };
        #[rustfmt::skip]
        writeln!(tw, "{}{}Merge\t{}{:>10}\t{}{:>5}\t{}{:>8}{}",
                 group_by,
                 st.pkg_p,
                 st.dur_p, fmt_duration(&fmtd, tottime),
                 st.cnt_p, totcount,
                 st.dur_p, fmt_duration(&fmtd, totavg), st.dur_s)?;
    }
    if show_sync && (print_zeros || !syncs.is_empty()) {
        let (time, avgcount, totcount) = syncs.iter().fold((0, 0, 0), |(tt, ac, tc), &t| {
                                                         if t < 0 {
                                                             (tt, ac, tc + 1)
                                                         } else {
                                                             (tt + t, ac + 1, tc + 1)
                                                         }
                                                     });
        let avg = if avgcount > 0 { time / avgcount } else { 0 };
        #[rustfmt::skip]
        writeln!(tw, "{}{}Sync\t{}{:>10}\t{}{:>5}\t{}{:>8}{}",
                 group_by,
                 st.pkg_p,
                 st.dur_p, fmt_duration(&fmtd, time),
                 st.cnt_p, totcount,
                 st.dur_p, fmt_duration(&fmtd, avg), st.dur_s)?;
    }
    Ok(())
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(tw: &mut TabWriter<Stdout>,
                   args: &ArgMatches,
                   subargs: &ArgMatches,
                   st: &Styles)
                   -> Result<bool, Error> {
    let now = epoch_now();
    let lim = value(subargs, "limit", parse_limit);
    let fmtd = value_t!(subargs, "duration", DurationStyle).unwrap();

    // Gather and print info about current merge process.
    let mut cms = std::i64::MAX;
    for i in get_all_info(Some("emerge"))? {
        cms = std::cmp::min(cms, i.start);
        #[rustfmt::skip]
        writeln!(tw, "Pid {}: ...{}\t{}{:>9}{}",
                 i.pid,
                 &i.cmdline[(i.cmdline.len()-35)..],
                 st.dur_p, fmt_duration(&fmtd, now-i.start), st.dur_s)?;
    }
    if cms == std::i64::MAX && atty::is(atty::Stream::Stdin) {
        writeln!(tw, "No ongoing merge found")?;
        return Ok(false);
    }

    // Parse emerge log.
    let hist = new_hist(myopen(args.value_of("logfile").unwrap())?,
                        args.value_of("logfile").unwrap().into(),
                        value_opt(args, "from", parse_date),
                        value_opt(args, "to", parse_date),
                        true,
                        false,
                        None,
                        false)?;
    let mut started: BTreeMap<(String, String), i64> = BTreeMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    for p in hist {
        match p {
            // We're ignoring iter here (reducing the start->stop matching accuracy) because there's no iter in the pretend output.
            ParsedHist::Start { ts, ebuild, version, .. } => {
                started.insert((ebuild.clone(), version.clone()), ts);
            },
            ParsedHist::Stop { ts, ebuild, version, .. } => {
                if let Some(start_ts) = started.remove(&(ebuild.clone(), version.clone())) {
                    let timevec = times.entry(ebuild.clone()).or_insert_with(|| vec![]);
                    timevec.insert(0, ts - start_ts);
                }
            },
            ParsedHist::SyncStart { .. } => (),
            ParsedHist::SyncStop { .. } => (),
        }
    }

    // Parse list of pending merges (from stdin or from emerge log filtered by cms).
    // We collect immediately to deal with type mismatches; it should be a small list anyway.
    let pretend: Vec<ParsedPretend> = if atty::is(atty::Stream::Stdin) {
        started.iter()
               .filter(|&(_, t)| *t > cms)
               .map(|(&(ref e, ref v), _)| ParsedPretend { ebuild: e.to_string(),
                                                           version: v.to_string() })
               .collect()
    } else {
        new_pretend(stdin(), "STDIN")
    };

    // Gather and print per-package and indivudual stats.
    let mut totcount = 0;
    let mut totunknown = 0;
    let mut totpredict = 0;
    let mut totelapsed = 0;
    for ParsedPretend { ebuild, version } in pretend {
        // Find the elapsed time, if any (heuristic is that emerge process started before
        // this merge finished, it's not failsafe but IMHO no worse than genlop).
        let (elapsed, elapsed_fmt) = match started.remove(&(ebuild.clone(), version.clone())) {
            Some(s) if s > cms => {
                (now - s, format!(" - {}{}{}", st.dur_p, fmt_duration(&fmtd, now - s), st.dur_s))
            },
            _ => (0, "".into()),
        };

        // Find the predicted time and adjust counters
        totcount += 1;
        let pred_fmt = match times.get(&ebuild) {
            Some(tv) => {
                let (predtime, predcount, _) = tv.iter().fold((0, 0, 0), |(pt, pc, tc), &i| {
                                                            if tc >= lim || i < 0 {
                                                                (pt, pc, tc + 1)
                                                            } else {
                                                                (pt + i, pc + 1, tc + 1)
                                                            }
                                                        });
                totpredict += predtime / predcount;
                if elapsed > 0 {
                    totelapsed += elapsed;
                    totpredict -= std::cmp::min(predtime / predcount, elapsed);
                }
                fmt_duration(&fmtd, predtime / predcount)
            },
            None => {
                totunknown += 1;
                "?".into()
            },
        };

        // Done
        #[rustfmt::skip]
        writeln!(tw, "{}{}-{}\t{}{:>9}{}{}",
                 st.pkg_p, ebuild, version,
                 st.dur_p, pred_fmt,
                 st.dur_s, elapsed_fmt)?;
    }
    if totcount > 0 {
        #[rustfmt::skip]
        writeln!(tw, "Estimate for {}{}{} ebuilds ({}{}{} unknown, {}{}{} elapsed)\t{}{:>9}{} @ {}{}{}",
                 st.cnt_p, totcount, st.cnt_s,
                 st.cnt_p, totunknown, st.cnt_s,
                 st.dur_p, fmt_duration(&fmtd, totelapsed), st.dur_s,
                 st.dur_p, fmt_duration(&fmtd, totpredict), st.dur_s,
                 st.dur_p, fmt_time(now + totpredict), st.dur_s)?;
    } else {
        writeln!(tw, "No pretended merge found")?;
    }
    Ok(totcount > 0)
}


#[cfg(test)]
mod tests {
    use super::{fmt_time, timespan_next, Timespan};
    use assert_cli::Assert;
    use chrono::{DateTime, Datelike, Local, TimeZone, Utc, Weekday};
    use regex::Regex;
    use std::{collections::HashMap,
              thread,
              time::{Duration, SystemTime, UNIX_EPOCH}};
    //TODO: Simplify fails_with() calls once https://github.com/assert-rs/assert_cli/issues/99 is closed

    /// Return the current time + offset. To make tests more reproducible, we wait until we're close
    /// to the start of a whole second before returning.
    fn ts(secs: i64) -> DateTime<Local> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        if now.subsec_millis() > 100 {
            thread::sleep(Duration::from_millis(25));
            ts(secs)
        } else {
            fmt_time(now.as_secs() as i64 + secs)
        }
    }

    #[test] #[rustfmt::skip]
    fn log() {
        let t: Vec<(&[&str], &str, i32)> = vec![
            // Basic test
            (&["-F", "test/emerge.10000.log", "l", "client"],
             "2018-02-04 04:55:19 +00:00     35:46 mail-client/thunderbird-52.6.0\n\
              2018-02-04 05:42:48 +00:00     47:29 www-client/firefox-58.0.1\n\
              2018-02-09 11:04:59 +00:00     47:58 mail-client/thunderbird-52.6.0-r1\n\
              2018-02-12 10:14:11 +00:00        31 kde-frameworks/kxmlrpcclient-5.43.0\n\
              2018-02-16 04:41:39 +00:00   6:03:14 www-client/chromium-64.0.3282.140\n\
              2018-02-19 17:35:41 +00:00   7:56:03 www-client/chromium-64.0.3282.167\n\
              2018-02-22 13:32:53 +00:00        44 www-client/links-2.14-r1\n\
              2018-02-28 09:14:37 +00:00      6:02 www-client/falkon-3.0.0\n\
              2018-03-06 04:19:52 +00:00   7:42:07 www-client/chromium-64.0.3282.186\n\
              2018-03-12 10:35:22 +00:00        14 x11-apps/xlsclients-1.1.4\n\
              2018-03-12 11:03:53 +00:00        16 kde-frameworks/kxmlrpcclient-5.44.0\n",
             0),
            // Check output when duration isn't known
            (&["-F", "test/emerge.10000.log", "l", "-s", "m", "mlt", "-e", "--from", "2018-02-18 12:37:00"],
             "2018-02-18 12:37:09 +00:00         ? media-libs/mlt-6.4.1-r6\n\
              2018-02-27 15:10:05 +00:00        43 media-libs/mlt-6.4.1-r6\n\
              2018-02-27 16:48:40 +00:00        39 media-libs/mlt-6.4.1-r6\n",
             0),
            // Check output of sync events
            (&["-F", "test/emerge.10000.log", "l", "-ss", "--from", "2018-03-07 10:42:00", "--to", "2018-03-07 14:00:00"],
             "2018-03-07 11:37:05 +00:00        38 Sync\n\
              2018-03-07 13:56:09 +00:00        40 Sync\n",
             0),
            (&["-F", "test/emerge.10000.log", "l", "--show", "ms", "--from", "2018-03-07 10:42:00", "--to", "2018-03-07 14:00:00"],
             "2018-03-07 10:43:10 +00:00        14 sys-apps/the_silver_searcher-2.0.0\n\
              2018-03-07 11:37:05 +00:00        38 Sync\n\
              2018-03-07 12:49:13 +00:00      1:01 sys-apps/util-linux-2.30.2-r1\n\
              2018-03-07 13:56:09 +00:00        40 Sync\n\
              2018-03-07 13:59:41 +00:00        24 dev-libs/nspr-4.18\n",
             0)
        ];
        for (a, o, e) in t {
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
        let t = vec![// Check garbage input
                     ("blah blah\n", format!("No pretended merge found\n"), 2),
                     // Check all-unknowns
                     ("[ebuild   R   ~] dev-lang/unknown-1.42\n",
                      format!("dev-lang/unknown-1.42                                  ?\n\
                               Estimate for 1 ebuilds (1 unknown, 0 elapsed)          0 @ {}\n",
                              ts(0)),
                      0),
                     // Check that unknown ebuild don't wreck alignment. Remember that times are {:>9}
                     ("[ebuild   R   ~] dev-qt/qtcore-5.9.4-r2\n\
                       [ebuild   R   ~] dev-lang/unknown-1.42\n\
                       [ebuild   R   ~] dev-qt/qtgui-5.9.4-r3\n",
                      format!("dev-qt/qtcore-5.9.4-r2                              3:44\n\
                               dev-lang/unknown-1.42                                  ?\n\
                               dev-qt/qtgui-5.9.4-r3                               4:36\n\
                               Estimate for 3 ebuilds (1 unknown, 0 elapsed)       8:20 @ {}\n",
                              ts(8 * 60 + 20)),
                      0),];
        for (i, o, e) in t {
            match e {
                0 => Assert::main_binary().with_args(&["-F", "test/emerge.10000.log", "p"])
                                          .stdin(i)
                                          .stdout()
                                          .is(o.as_str())
                                          .unwrap(),
                _ => Assert::main_binary().with_args(&["-F", "test/emerge.10000.log", "p"])
                                          .fails_with(e)
                                          .stdin(i)
                                          .stdout()
                                          .is(o.as_str())
                                          .unwrap(),
            }
        }
    }

    #[test] #[rustfmt::skip]
    fn stats() {
        let t: Vec<(&[&str],&str,i32)> = vec![
            (&["-F","test/emerge.10000.log","s","client"],
             "kde-frameworks/kxmlrpcclient          47      2        23\n\
              mail-client/thunderbird          1:23:44      2     41:52\n\
              www-client/chromium             21:41:24      3   7:13:48\n\
              www-client/falkon                   6:02      1      6:02\n\
              www-client/firefox                 47:29      1     47:29\n\
              www-client/links                      44      1        44\n\
              x11-apps/xlsclients                   14      1        14\n",
             0),
            (&["-F","test/emerge.10000.log","s","client","-ss"],
             "Sync     1:19:28    150        31\n",
             0),
            (&["-F","test/emerge.10000.log","s","client","-sst"],
             "Merge    24:00:24     11   2:10:56\n\
              Sync      1:19:28    150        31\n",
             0),
            (&["-F","test/emerge.10000.log","s","client","-sa"],
             "kde-frameworks/kxmlrpcclient          47      2        23\n\
              mail-client/thunderbird          1:23:44      2     41:52\n\
              www-client/chromium             21:41:24      3   7:13:48\n\
              www-client/falkon                   6:02      1      6:02\n\
              www-client/firefox                 47:29      1     47:29\n\
              www-client/links                      44      1        44\n\
              x11-apps/xlsclients                   14      1        14\n\
              Merge                           24:00:24     11   2:10:56\n\
              Sync                             1:19:28    150        31\n",
             0),
            (&["-F","test/emerge.10000.log","s","--from","2018-02-03T23:11:47","--to","2018-02-04","notfound","-sa"],
             "Merge           0      0         0\n\
              Sync            0      0         0\n",
             2),
        ];
        for (a, o, e) in t {
            match e {
                0 => Assert::main_binary().with_args(a).stdout().is(o).unwrap(),
                _ => Assert::main_binary().with_args(a).fails_with(e).stdout().is(o).unwrap(),
            }
        }
    }

    /// Test grouped stats. In addition to the usual check that the actual output matches the
    /// expected one, we check that the expected outputs are consistent (y/m/w/d totals are the
    /// same, and avg*count==tot).
    #[test] #[rustfmt::skip]
    fn stats_grouped() {
        let t: Vec<(&[&str],&str)> = vec![
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gy"],
             "2018 sys-kernel/gentoo-sources         904     10        90\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gm"],
             "2018-02 sys-kernel/gentoo-sources         702      8        87\n\
              2018-03 sys-kernel/gentoo-sources         202      2       101\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gw"],
             "2018-05 sys-kernel/gentoo-sources          81      1        81\n\
              2018-06 sys-kernel/gentoo-sources         192      2        96\n\
              2018-07 sys-kernel/gentoo-sources         198      2        99\n\
              2018-08 sys-kernel/gentoo-sources          77      1        77\n\
              2018-09 sys-kernel/gentoo-sources         236      3        78\n\
              2018-11 sys-kernel/gentoo-sources         120      1       120\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gd"],
             "2018-02-04 sys-kernel/gentoo-sources          81      1        81\n\
              2018-02-05 sys-kernel/gentoo-sources          95      1        95\n\
              2018-02-08 sys-kernel/gentoo-sources          97      1        97\n\
              2018-02-12 sys-kernel/gentoo-sources          80      1        80\n\
              2018-02-18 sys-kernel/gentoo-sources         118      1       118\n\
              2018-02-23 sys-kernel/gentoo-sources          77      1        77\n\
              2018-02-26 sys-kernel/gentoo-sources          79      1        79\n\
              2018-02-28 sys-kernel/gentoo-sources          75      1        75\n\
              2018-03-01 sys-kernel/gentoo-sources          82      1        82\n\
              2018-03-12 sys-kernel/gentoo-sources         120      1       120\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gy"],
             "2018 Merge      216426    831       260\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gm"],
             "2018-02 Merge      158312    533       297\n\
              2018-03 Merge       58114    298       195\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gw"],
             "2018-05 Merge       33577     63       532\n\
              2018-06 Merge       10070     74       136\n\
              2018-07 Merge       58604    281       208\n\
              2018-08 Merge       51276     65       788\n\
              2018-09 Merge       14737     71       207\n\
              2018-10 Merge       43782    182       240\n\
              2018-11 Merge        4380     95        46\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gd"],
             "2018-02-03 Merge        2741     32        85\n\
              2018-02-04 Merge       30836     31       994\n\
              2018-02-05 Merge         158      4        39\n\
              2018-02-06 Merge        4288     44        97\n\
              2018-02-07 Merge         857     15        57\n\
              2018-02-08 Merge         983      5       196\n\
              2018-02-09 Merge        3784      6       630\n\
              2018-02-12 Merge       29239    208       140\n\
              2018-02-13 Merge          19      1        19\n\
              2018-02-14 Merge        4795     44       108\n\
              2018-02-15 Merge         137      3        45\n\
              2018-02-16 Merge       23914     21      1138\n\
              2018-02-18 Merge         500      4       125\n\
              2018-02-19 Merge       28977      2     14488\n\
              2018-02-20 Merge         488      2       244\n\
              2018-02-21 Merge        5522     37       149\n\
              2018-02-22 Merge       15396     16       962\n\
              2018-02-23 Merge         854      6       142\n\
              2018-02-24 Merge          39      2        19\n\
              2018-02-26 Merge        2730     10       273\n\
              2018-02-27 Merge        1403     35        40\n\
              2018-02-28 Merge         652      5       130\n\
              2018-03-01 Merge        9355     13       719\n\
              2018-03-02 Merge         510      5       102\n\
              2018-03-03 Merge          87      3        29\n\
              2018-03-05 Merge         168      9        18\n\
              2018-03-06 Merge       27746      3      9248\n\
              2018-03-07 Merge        2969     46        64\n\
              2018-03-08 Merge        5441     74        73\n\
              2018-03-09 Merge        7458     50       149\n\
              2018-03-12 Merge        4380     95        46\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gy"],
             "2018 Sync        4768    150        31\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gm"],
             "2018-02 Sync        2429     90        26\n\
              2018-03 Sync        2339     60        38\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gw"],
             "2018-05 Sync         162      3        54\n\
              2018-06 Sync         957     31        30\n\
              2018-07 Sync         391     17        23\n\
              2018-08 Sync         503     20        25\n\
              2018-09 Sync        1906     39        48\n\
              2018-10 Sync         728     36        20\n\
              2018-11 Sync         121      4        30\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gd"],
             "2018-02-03 Sync          69      1        69\n\
              2018-02-04 Sync          93      2        46\n\
              2018-02-05 Sync         188      7        26\n\
              2018-02-06 Sync         237      7        33\n\
              2018-02-07 Sync         223      7        31\n\
              2018-02-08 Sync         217      7        31\n\
              2018-02-09 Sync          92      3        30\n\
              2018-02-12 Sync          87      4        21\n\
              2018-02-13 Sync          46      2        23\n\
              2018-02-14 Sync          85      3        28\n\
              2018-02-15 Sync          77      4        19\n\
              2018-02-16 Sync          68      3        22\n\
              2018-02-18 Sync          28      1        28\n\
              2018-02-19 Sync          61      2        30\n\
              2018-02-20 Sync         120      5        24\n\
              2018-02-21 Sync          90      4        22\n\
              2018-02-22 Sync          51      2        25\n\
              2018-02-23 Sync         158      6        26\n\
              2018-02-24 Sync          23      1        23\n\
              2018-02-26 Sync          69      4        17\n\
              2018-02-27 Sync         211      8        26\n\
              2018-02-28 Sync         136      7        19\n\
              2018-03-01 Sync         569      8        71\n\
              2018-03-02 Sync         548     10        54\n\
              2018-03-03 Sync         373      2       186\n\
              2018-03-05 Sync          46      9         5\n\
              2018-03-06 Sync         183      8        22\n\
              2018-03-07 Sync         120      4        30\n\
              2018-03-08 Sync         157      8        19\n\
              2018-03-09 Sync         222      7        31\n\
              2018-03-12 Sync         121      4        30\n"),
        ];
        let re = Regex::new("([0-9]+) +([0-9]+) +([0-9]+)$").unwrap();
        let mut tots_t: HashMap<&str, i32> = HashMap::new();
        let mut tots_c: HashMap<&str, i32> = HashMap::new();
        for (a, o) in t {
            for l in o.lines() {
                let cap = re.captures(&l).expect("Line doesn't match regex");
                let cap_t = cap.get(1).unwrap().as_str().parse::<i32>().expect("Can't parse tottime");
                let cap_c = cap.get(2).unwrap().as_str().parse::<i32>().expect("Can't parse count");
                let cap_a = cap.get(3).unwrap().as_str().parse::<i32>().expect("Can't parse avgtime");
                assert!((cap_t - cap_c * cap_a).abs() < cap_c, "Average*count should match total: {}", l);
                let tot_t = tots_t.entry(a.last().unwrap()).or_insert(0);
                *tot_t += cap_t;
                let tot_c = tots_c.entry(a.last().unwrap()).or_insert(0);
                *tot_c += cap_c;
            }
            Assert::main_binary().with_args(a).stdout().is(o).unwrap();
        }
        assert!(tots_c.iter().all(|(_,c)| c == tots_c.get("-gy").unwrap()), "Total count should match {:?}", tots_c);
        assert!(tots_t.iter().all(|(_,t)| t == tots_t.get("-gy").unwrap()), "Total times should match {:?}", tots_t);
    }

    /// Test behaviour when clock goes backward between merge start and merge end. Likely to happen
    /// when you're bootstrapping an Gentoo and setting the time halfway through.
    #[test]
    fn negative_merge_time() {
        for (a, i, o) in vec![// For `log` we show an unknown time.
                 (vec!["-F", "test/emerge.negtime.log", "l", "-sa"],
                  "",
                  format!("2019-06-05 09:32:10 +01:00      1:09 Sync\n\
                           2019-06-05 12:26:54 +01:00      5:56 kde-plasma/kwin-5.15.5\n\
                           2019-06-06 03:11:48 +01:00        26 kde-apps/libktnef-19.04.1\n\
                           2019-06-06 03:16:01 +01:00        34 net-misc/chrony-3.3\n\
                           2019-06-05 11:18:28 +01:00         ? Sync\n\
                           2019-06-05 11:21:02 +01:00         ? kde-plasma/kwin-5.15.5\n\
                           2019-06-08 22:33:36 +01:00      3:10 kde-plasma/kwin-5.15.5\n")),
                 // For `pred` the negative merge time is ignored.
                 (vec!["-F", "test/emerge.negtime.log", "p"],
                  "[ebuild   R   ~] kde-plasma/kwin-5.15.5\n",
                  format!("kde-plasma/kwin-5.15.5                              4:33\n\
                           Estimate for 1 ebuilds (0 unknown, 0 elapsed)       4:33 @ {}\n",
                          ts(4 * 60 + 33))),
                 // For `stats` the negative merge time is used for count but ignored for tottime/predtime.
                 (vec!["-F", "test/emerge.negtime.log", "s", "-sa"],
                  "",
                  format!("kde-apps/libktnef          26      1        26\n\
                           kde-plasma/kwin          9:06      3      4:33\n\
                           net-misc/chrony            34      1        34\n\
                           Merge                   10:06      5      2:01\n\
                           Sync                     1:09      2      1:09\n")),]
        {
            Assert::main_binary().with_args(&a).stdin(i).stdout().is(o.as_str()).unwrap();
        }
    }

    #[test]
    fn exit_status() {
        // 0: no problem
        // 1: user or program error
        // 2: command ran properly but didn't find anything
        let t: Vec<(&[&str], i32)> =
            vec![// Help, version, badarg (clap)
                 (&["-h"], 0),
                 (&["-V"], 0),
                 (&["l", "-h"], 0),
                 (&[], 1),
                 (&["s", "--foo"], 1),
                 (&["badcmd"], 1),
                 // Bad arguments (emlop)
                 (&["l", "--logfile", "notfound"], 1),
                 (&["s", "--logfile", "notfound"], 1),
                 (&["p", "--logfile", "notfound"], 1),
                 (&["l", "bad regex [a-z"], 1),
                 (&["s", "bad regex [a-z"], 1),
                 (&["p", "bad regex [a-z"], 1),
                 // Normal behaviour
                 (&["-F", "test/emerge.10000.log", "p"], 2),
                 (&["-F", "test/emerge.10000.log", "l"], 0),
                 (&["-F", "test/emerge.10000.log", "l", "-s"], 0),
                 (&["-F", "test/emerge.10000.log", "l", "-e", "icu"], 0),
                 (&["-F", "test/emerge.10000.log", "l", "-e", "unknown"], 2),
                 (&["-F", "test/emerge.10000.log", "l", "--from", "2018-09-28"], 2),
                 (&["-F", "test/emerge.10000.log", "l", "-s", "--from", "2018-09-28"], 2),
                 (&["-F", "test/emerge.10000.log", "s"], 0),
                 (&["-F", "test/emerge.10000.log", "s", "-e", "icu"], 0),
                 (&["-F", "test/emerge.10000.log", "s", "-e", "unknown"], 2),];
        for (args, exit) in t {
            match exit {
                0 => Assert::main_binary().with_args(args).unwrap(),
                _ => Assert::main_binary().with_args(args).fails_with(exit).unwrap(),
            }
        }
    }

    #[test]
    fn timespan_next_() {
        for t in &[// input                   year       month      week       day
                   "2019-01-01T00:00:00+00:00 2020-01-01 2019-02-01 2019-01-07 2019-01-02",
                   "2019-01-01T23:59:59+00:00 2020-01-01 2019-02-01 2019-01-07 2019-01-02",
                   "2019-01-30T00:00:00+00:00 2020-01-01 2019-02-01 2019-02-04 2019-01-31",
                   "2019-01-31T00:00:00+00:00 2020-01-01 2019-02-01 2019-02-04 2019-02-01",
                   "2019-12-31T00:00:00+00:00 2020-01-01 2020-01-01 2020-01-06 2020-01-01",
                   "2020-02-28T12:34:00+00:00 2021-01-01 2020-03-01 2020-03-02 2020-02-29"]
        {
            let v: Vec<&str> = t.split_whitespace().collect();
            let i = DateTime::parse_from_rfc3339(v[0]).unwrap().timestamp();
            let y = DateTime::parse_from_rfc3339(&format!("{}T00:00:00+00:00", v[1])).unwrap();
            let m = DateTime::parse_from_rfc3339(&format!("{}T00:00:00+00:00", v[2])).unwrap();
            let w = DateTime::parse_from_rfc3339(&format!("{}T00:00:00+00:00", v[3])).unwrap();
            let d = DateTime::parse_from_rfc3339(&format!("{}T00:00:00+00:00", v[4])).unwrap();
            assert_eq!(y, Utc.timestamp(timespan_next(i, &Timespan::Year), 0), "year {}", v[0]);
            assert_eq!(m, Utc.timestamp(timespan_next(i, &Timespan::Month), 0), "month {}", v[0]);
            assert_eq!(w, Utc.timestamp(timespan_next(i, &Timespan::Week), 0), "week {}", v[0]);
            assert_eq!(Weekday::Mon, Utc.timestamp(timespan_next(i, &Timespan::Week), 0).weekday());
            assert_eq!(d, Utc.timestamp(timespan_next(i, &Timespan::Day), 0), "day {}", v[0]);
        }
    }
}
