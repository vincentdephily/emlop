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
    let show_unmerge = show.contains(&"u") || show.contains(&"a");
    let hist = new_hist(args.value_of("logfile").unwrap().into(),
                        value_opt(args, "from", parse_date),
                        value_opt(args, "to", parse_date),
                        show_merge,
                        show_unmerge,
                        show_sync,
                        subargs.value_of("package"),
                        subargs.is_present("exact"))?;
    let fmtd = value_t!(subargs, "duration", DurationStyle).unwrap();
    let mut merges: HashMap<(String, String, String), i64> = HashMap::new();
    let mut unmerges: HashMap<(String, String), i64> = HashMap::new();
    let mut found_one = false;
    let mut syncstart: i64 = 0;
    for p in hist {
        match p {
            ParsedHist::Start { ts, ebuild, version, iter, .. } => {
                // This'll overwrite any previous entry, if a merge started but never finished
                merges.insert((ebuild, version, iter), ts);
            },
            ParsedHist::Stop { ts, ebuild, version, iter, .. } => {
                found_one = true;
                let k = (ebuild, version, iter);
                let started = merges.remove(&k);
                let (ebuild, version, _) = k;
                #[rustfmt::skip]
                writeln!(stdout(), "{} {}{:>9} {}{}-{}{}",
                         fmt_time(ts),
                         st.dur_p, fmt_duration(&fmtd, ts - started.unwrap_or(ts + 1)),
                         st.merge_p, ebuild, version, st.merge_s).unwrap_or(());
            },
            ParsedHist::UnmergeStart { ts, ebuild, version, .. } => {
                // This'll overwrite any previous entry, if a build started but never finished
                unmerges.insert((ebuild, version), ts);
            },
            ParsedHist::UnmergeStop { ts, ebuild, version, .. } => {
                found_one = true;
                let k = (ebuild, version);
                let started = unmerges.remove(&k);
                let (ebuild, version) = k;
                #[rustfmt::skip]
                writeln!(stdout(), "{} {}{:>9} {}{}-{}{}",
                         fmt_time(ts),
                         st.dur_p, fmt_duration(&fmtd, ts - started.unwrap_or(ts + 1)),
                         st.unmerge_p, ebuild, version, st.unmerge_s).unwrap_or(());
            },
            ParsedHist::SyncStart { ts } => {
                syncstart = ts;
            },
            ParsedHist::SyncStop { ts } => {
                found_one = true;
                #[rustfmt::skip]
                writeln!(stdout(), "{} {}{:>9}{} Sync",
                         fmt_time(ts),
                         st.dur_p, fmt_duration(&fmtd, ts - syncstart), st.dur_s).unwrap_or(());
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

/// Wrapper to extract stats from a list of data points (durations).
struct Times {
    vals: Vec<i64>,
    count: i64,
    tot: i64,
}
impl Times {
    fn new() -> Self {
        Self { vals: vec![], count: 0, tot: 0 }
    }
    /// Digest new data point
    ///
    /// Data points should be inserted in chronological order.
    /// We don't store negative values but we still take them into account.
    fn insert(&mut self, t: i64) {
        self.count += 1;
        if t > 0 {
            self.vals.insert(0, t); // FIXME: append would be cheaper ?
            self.tot += t;
        }
    }
    fn is_empty(&self) -> bool {
        self.count == 0
    }
    fn clear(&mut self) {
        self.vals.clear();
        self.count = 0;
        self.tot = 0;
    }
    /// Predict the next data point by looking at past ones
    fn pred(&self, lim: u16) -> i64 {
        let (t, c) = self.vals.iter().take(lim as usize).fold((0, 0), |(t, c), v| (t + v, c + 1));
        if c > 0 {
            t / c
        } else {
            -1 // FIXME Return None
        }
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
    let show_pkt = show.contains(&"m") || show.contains(&"u") || show.contains(&"a"); // FIXME: split m/u
    let show_tot = show.contains(&"t") || show.contains(&"a");
    let show_sync = show.contains(&"s") || show.contains(&"a");
    let timespan_opt = value_opt(subargs, "group", parse_timespan);
    let hist = new_hist(args.value_of("logfile").unwrap().into(),
                        value_opt(args, "from", parse_date),
                        value_opt(args, "to", parse_date),
                        show_pkt || show_tot,
                        show_pkt || show_tot,
                        show_sync,
                        subargs.value_of("package"),
                        subargs.is_present("exact"))?;
    let fmtd = value_t!(subargs, "duration", DurationStyle).unwrap();
    let lim = value(subargs, "limit", parse_limit);
    let mut merge_start: HashMap<(String, String, String), i64> = HashMap::new();
    let mut unmerge_start: HashMap<(String, String), i64> = HashMap::new();
    let mut pkt_time: BTreeMap<String, (Times, Times)> = BTreeMap::new();
    let mut sync_start: i64 = 0;
    let mut sync_time = Times::new();
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
                cmd_stats_group(tw, &st, &fmtd, lim, show_pkt, show_tot, show_sync, &group_by,
                                &sync_time, &pkt_time)?;
                sync_time.clear();
                pkt_time.clear();
                nextts = timespan_next(t, timespan);
                curts = t;
            }
        }
        match p {
            ParsedHist::Start { ts, ebuild, version, iter, .. } => {
                merge_start.insert((ebuild, version, iter), ts);
            },
            ParsedHist::Stop { ts, ebuild, version, iter, .. } => {
                let k = (ebuild, version, iter);
                if let Some(start_ts) = merge_start.remove(&k) {
                    let (times, _) =
                        pkt_time.entry(k.0).or_insert_with(|| (Times::new(), Times::new()));
                    times.insert(ts - start_ts);
                }
            },
            ParsedHist::UnmergeStart { ts, ebuild, version } => {
                unmerge_start.insert((ebuild, version), ts);
            },
            ParsedHist::UnmergeStop { ts, ebuild, version } => {
                let k = (ebuild, version);
                if let Some(start_ts) = unmerge_start.remove(&k) {
                    let (_, times) =
                        pkt_time.entry(k.0).or_insert_with(|| (Times::new(), Times::new()));
                    times.insert(ts - start_ts);
                }
            },
            ParsedHist::SyncStart { ts } => {
                sync_start = ts;
            },
            ParsedHist::SyncStop { ts } => {
                sync_time.insert(ts - sync_start);
            },
        }
    }
    let group_by = timespan_opt.map_or(String::new(), |t| timespan_header(curts, &t));
    cmd_stats_group(tw, &st, &fmtd, lim, show_pkt, show_tot, show_sync, &group_by, &sync_time,
                    &pkt_time)?;
    Ok(!pkt_time.is_empty() || !sync_time.is_empty())
}

fn cmd_stats_group(tw: &mut TabWriter<Stdout>,
                   st: &Styles,
                   fmtd: &DurationStyle,
                   lim: u16,
                   show_pkt: bool,
                   show_tot: bool,
                   show_sync: bool,
                   group_by: &str,
                   sync_time: &Times,
                   pkt_time: &BTreeMap<String, (Times, Times)>)
                   -> Result<(), Error> {
    if show_pkt && !pkt_time.is_empty() {
        for (pkg, (merge, unmerge)) in pkt_time {
            #[rustfmt::skip]
            writeln!(tw, "{}{}{}\t{}{:>5}\t{}{:>10}\t{}{:>8}\t{}{:>5}\t{}{:>8}\t{}{:>8}{}",
                     group_by,
                     st.pkg_p, pkg,
                     st.cnt_p, merge.count,
                     st.dur_p, fmt_duration(&fmtd, merge.tot),
                     st.dur_p, fmt_duration(&fmtd, merge.pred(lim)),
                     st.cnt_p, unmerge.count,
                     st.dur_p, fmt_duration(&fmtd, unmerge.tot),
                     st.dur_p, fmt_duration(&fmtd, unmerge.pred(lim)),
                     st.dur_s)?;
        }
    }
    if show_tot && !pkt_time.is_empty() {
        let mut merge_time = 0;
        let mut merge_count = 0;
        let mut unmerge_time = 0;
        let mut unmerge_count = 0;
        for (merge, unmerge) in pkt_time.values() {
            merge_time += merge.tot;
            merge_count += merge.count;
            unmerge_time += unmerge.tot;
            unmerge_count += unmerge.count;
        }
        #[rustfmt::skip]
        writeln!(tw, "{}Total\t{}{:>5}\t{}{:>10}\t{}{:>8}\t{}{:>5}\t{}{:>8}\t{}{:>8}{}",
                 group_by,
                 st.cnt_p, merge_count,
                 st.dur_p, fmt_duration(&fmtd, merge_time),
                 st.dur_p, fmt_duration(&fmtd, merge_time.checked_div(merge_count).unwrap_or(-1)),
                 st.cnt_p, unmerge_count,
                 st.dur_p, fmt_duration(&fmtd, unmerge_time),
                 st.dur_p, fmt_duration(&fmtd, unmerge_time.checked_div(unmerge_count).unwrap_or(-1)),
                 st.dur_s)?;
    }
    if show_sync && !sync_time.is_empty() {
        #[rustfmt::skip]
        writeln!(tw, "{}Sync\t{}{:>5}\t{}{:>10}\t{}{:>8}{}",
                 group_by,
                 st.cnt_p, sync_time.count,
                 st.dur_p, fmt_duration(&fmtd, sync_time.tot),
                 st.dur_p, fmt_duration(&fmtd, sync_time.pred(lim)),
                 st.dur_s)?;
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
    let hist = new_hist(args.value_of("logfile").unwrap().into(),
                        value_opt(args, "from", parse_date),
                        value_opt(args, "to", parse_date),
                        true,
                        false,
                        false,
                        None,
                        false)?;
    let mut started: BTreeMap<(String, String), i64> = BTreeMap::new();
    let mut times: HashMap<String, Times> = HashMap::new();
    for p in hist {
        match p {
            // We're ignoring iter here (reducing the start->stop matching accuracy) because there's no iter in the pretend output.
            ParsedHist::Start { ts, ebuild, version, .. } => {
                started.insert((ebuild, version), ts);
            },
            ParsedHist::Stop { ts, ebuild, version, .. } => {
                let k = (ebuild, version);
                if let Some(start_ts) = started.remove(&k) {
                    let timevec = times.entry(k.0).or_insert_with(|| Times::new());
                    timevec.insert(ts - start_ts);
                }
            },
            ParsedHist::UnmergeStart { .. } => (),
            ParsedHist::UnmergeStop { .. } => (),
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
        let k = (ebuild, version);
        let (elapsed, elapsed_fmt) = match started.remove(&k) {
            Some(s) if s > cms => {
                (now - s, format!(" - {}{}{}", st.dur_p, fmt_duration(&fmtd, now - s), st.dur_s))
            },
            _ => (0, "".into()),
        };
        let (ebuild, version) = k;

        // Find the predicted time and adjust counters
        totcount += 1;
        let pred_fmt = match times.get(&ebuild) {
            Some(tv) => {
                let pred = tv.pred(lim);
                totpredict += pred;
                if elapsed > 0 {
                    totelapsed += elapsed;
                    totpredict -= std::cmp::min(pred, elapsed);
                }
                fmt_duration(&fmtd, pred)
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

    #[test]
    fn log() {
        #[rustfmt::skip]
        let t: Vec<(&[&str], &str, i32)> = vec![
            // Basic test
            (&["-F", "test/emerge.10000.log", "l", "client"],
             "2018-02-04 04:55:19 +00:00     35:46 >>> mail-client/thunderbird-52.6.0\n\
              2018-02-04 05:42:48 +00:00     47:29 >>> www-client/firefox-58.0.1\n\
              2018-02-09 11:04:59 +00:00     47:58 >>> mail-client/thunderbird-52.6.0-r1\n\
              2018-02-12 10:14:11 +00:00        31 >>> kde-frameworks/kxmlrpcclient-5.43.0\n\
              2018-02-16 04:41:39 +00:00   6:03:14 >>> www-client/chromium-64.0.3282.140\n\
              2018-02-19 17:35:41 +00:00   7:56:03 >>> www-client/chromium-64.0.3282.167\n\
              2018-02-22 13:32:53 +00:00        44 >>> www-client/links-2.14-r1\n\
              2018-02-28 09:14:37 +00:00      6:02 >>> www-client/falkon-3.0.0\n\
              2018-03-06 04:19:52 +00:00   7:42:07 >>> www-client/chromium-64.0.3282.186\n\
              2018-03-12 10:35:22 +00:00        14 >>> x11-apps/xlsclients-1.1.4\n\
              2018-03-12 11:03:53 +00:00        16 >>> kde-frameworks/kxmlrpcclient-5.44.0\n",
             0),
            // Check output when duration isn't known
            (&["-F", "test/emerge.10000.log", "l", "-s", "m", "mlt", "-e", "--from", "2018-02-18 12:37:00"],
             "2018-02-18 12:37:09 +00:00         ? >>> media-libs/mlt-6.4.1-r6\n\
              2018-02-27 15:10:05 +00:00        43 >>> media-libs/mlt-6.4.1-r6\n\
              2018-02-27 16:48:40 +00:00        39 >>> media-libs/mlt-6.4.1-r6\n",
             0),
            // Check output of sync events
            (&["-F", "test/emerge.10000.log", "l", "-ss", "--from", "2018-03-07 10:42:00", "--to", "2018-03-07 14:00:00"],
             "2018-03-07 11:37:05 +00:00        38 Sync\n\
              2018-03-07 13:56:09 +00:00        40 Sync\n",
             0),
            // Check output of all events
            (&["-F", "test/emerge.10000.log", "l", "--show", "a", "--from", "2018-03-07 10:42:00", "--to", "2018-03-07 14:00:00"],
             "2018-03-07 10:43:10 +00:00        14 >>> sys-apps/the_silver_searcher-2.0.0\n\
              2018-03-07 11:37:05 +00:00        38 Sync\n\
              2018-03-07 12:49:09 +00:00         2 <<< sys-apps/util-linux-2.30.2\n\
              2018-03-07 12:49:13 +00:00      1:01 >>> sys-apps/util-linux-2.30.2-r1\n\
              2018-03-07 13:56:09 +00:00        40 Sync\n\
              2018-03-07 13:59:38 +00:00         2 <<< dev-libs/nspr-4.17\n\
              2018-03-07 13:59:41 +00:00        24 >>> dev-libs/nspr-4.18\n",
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

    #[test]
    fn stats() {
        #[rustfmt::skip]
        let t: Vec<(&[&str],&str,i32)> = vec![
            (&["-F","test/emerge.10000.log","s","client"],
             "kde-frameworks/kxmlrpcclient      2          47        23      2         4         2\n\
              mail-client/thunderbird           2     1:23:44     41:52      2         6         3\n\
              www-client/chromium               3    21:41:24   7:13:48      3        12         4\n\
              www-client/falkon                 1        6:02      6:02      0         0         ?\n\
              www-client/firefox                1       47:29     47:29      1         3         3\n\
              www-client/links                  1          44        44      1         1         1\n\
              x11-apps/xlsclients               1          14        14      1         1         1\n",
             0),
            (&["-F","test/emerge.10000.log","s","client","-ss"],
             "Sync    150     1:19:28        30\n",
             0),
            (&["-F","test/emerge.10000.log","s","client","-sst"],
             "Total     11    24:00:24   2:10:56     10        27         2\n\
              Sync     150     1:19:28        30\n",
             0),
            (&["-F","test/emerge.10000.log","s","client","-sa"],
             "kde-frameworks/kxmlrpcclient      2          47        23      2         4         2\n\
              mail-client/thunderbird           2     1:23:44     41:52      2         6         3\n\
              www-client/chromium               3    21:41:24   7:13:48      3        12         4\n\
              www-client/falkon                 1        6:02      6:02      0         0         ?\n\
              www-client/firefox                1       47:29     47:29      1         3         3\n\
              www-client/links                  1          44        44      1         1         1\n\
              x11-apps/xlsclients               1          14        14      1         1         1\n\
              Total                            11    24:00:24   2:10:56     10        27         2\n\
              Sync                            150     1:19:28        30\n",
             0),
            (&["-F","test/emerge.10000.log","s","--from","2018-02-03T23:11:47","--to","2018-02-04","notfound","-sa"],
             "",
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
    #[test]
    fn stats_grouped() {
        #[rustfmt::skip]
        let t: Vec<(&[&str],&str)> = vec![
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gy"],
             "2018 sys-kernel/gentoo-sources     10         904        90     11       200        16\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gm"],
             "2018-02 sys-kernel/gentoo-sources      8         702        87      8       149        18\n\
              2018-03 sys-kernel/gentoo-sources      2         202       101      3        51        17\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gw"],
             "2018-05 sys-kernel/gentoo-sources      1          81        81      0         0         ?\n\
              2018-06 sys-kernel/gentoo-sources      2         192        96      3        66        22\n\
              2018-07 sys-kernel/gentoo-sources      2         198        99      0         0         ?\n\
              2018-08 sys-kernel/gentoo-sources      1          77        77      3        37        12\n\
              2018-09 sys-kernel/gentoo-sources      3         236        78      3        61        20\n\
              2018-10 sys-kernel/gentoo-sources      0           0         ?      1        23        23\n\
              2018-11 sys-kernel/gentoo-sources      1         120       120      1        13        13\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-sm","gentoo-sources","-gd"],
             "2018-02-04 sys-kernel/gentoo-sources      1          81        81      0         0         ?\n\
              2018-02-05 sys-kernel/gentoo-sources      1          95        95      0         0         ?\n\
              2018-02-06 sys-kernel/gentoo-sources      0           0         ?      3        66        22\n\
              2018-02-08 sys-kernel/gentoo-sources      1          97        97      0         0         ?\n\
              2018-02-12 sys-kernel/gentoo-sources      1          80        80      0         0         ?\n\
              2018-02-18 sys-kernel/gentoo-sources      1         118       118      0         0         ?\n\
              2018-02-22 sys-kernel/gentoo-sources      0           0         ?      3        37        12\n\
              2018-02-23 sys-kernel/gentoo-sources      1          77        77      0         0         ?\n\
              2018-02-26 sys-kernel/gentoo-sources      1          79        79      0         0         ?\n\
              2018-02-27 sys-kernel/gentoo-sources      0           0         ?      2        46        23\n\
              2018-02-28 sys-kernel/gentoo-sources      1          75        75      0         0         ?\n\
              2018-03-01 sys-kernel/gentoo-sources      1          82        82      1        15        15\n\
              2018-03-05 sys-kernel/gentoo-sources      0           0         ?      1        23        23\n\
              2018-03-12 sys-kernel/gentoo-sources      1         120       120      1        13        13\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gy"],
             "2018 Total    831      216426       260    832      2311         2\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gm"],
             "2018-02 Total    533      158312       297    529      1497         2\n\
              2018-03 Total    298       58114       195    303       814         2\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gw"],
             "2018-05 Total     63       33577       532     60       132         2\n\
              2018-06 Total     74       10070       136     68       225         3\n\
              2018-07 Total    281       58604       208    258       709         2\n\
              2018-08 Total     65       51276       788     69       197         2\n\
              2018-09 Total     71       14737       207     95       316         3\n\
              2018-10 Total    182       43782       240    187       519         2\n\
              2018-11 Total     95        4380        46     95       213         2\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-st","-gd"],
             "2018-02-03 Total     32        2741        85     32        70         2\n\
              2018-02-04 Total     31       30836       994     28        62         2\n\
              2018-02-05 Total      4         158        39      3         5         1\n\
              2018-02-06 Total     44        4288        97     44       174         3\n\
              2018-02-07 Total     15         857        57     13        28         2\n\
              2018-02-08 Total      5         983       196      4         8         2\n\
              2018-02-09 Total      6        3784       630      4        10         2\n\
              2018-02-12 Total    208       29239       140    206       587         2\n\
              2018-02-13 Total      1          19        19      0         0         ?\n\
              2018-02-14 Total     44        4795       108     44        92         2\n\
              2018-02-15 Total      3         137        45      3         6         2\n\
              2018-02-16 Total     21       23914      1138      3        14         4\n\
              2018-02-18 Total      4         500       125      2        10         5\n\
              2018-02-19 Total      2       28977     14488      2         6         3\n\
              2018-02-20 Total      2         488       244      1         2         2\n\
              2018-02-21 Total     37        5522       149     36        93         2\n\
              2018-02-22 Total     16       15396       962     23        82         3\n\
              2018-02-23 Total      6         854       142      5        11         2\n\
              2018-02-24 Total      2          39        19      2         3         1\n\
              2018-02-26 Total     10        2730       273      9        18         2\n\
              2018-02-27 Total     35        1403        40     49       175         3\n\
              2018-02-28 Total      5         652       130     16        41         2\n\
              2018-03-01 Total     13        9355       719     13        40         3\n\
              2018-03-02 Total      5         510       102      5        37         7\n\
              2018-03-03 Total      3          87        29      3         5         1\n\
              2018-03-05 Total      9         168        18     21        84         4\n\
              2018-03-06 Total      3       27746      9248      1         3         3\n\
              2018-03-07 Total     46        2969        64     43        90         2\n\
              2018-03-08 Total     74        5441        73     73       202         2\n\
              2018-03-09 Total     50        7458       149     49       140         2\n\
              2018-03-12 Total     95        4380        46     95       213         2\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gy"],
             "2018 Sync    150        4768        30\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gm"],
             "2018-02 Sync     90        2429        18\n\
              2018-03 Sync     60        2339        30\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gw"],
             "2018-05 Sync      3         162        54\n\
              2018-06 Sync     31         957        30\n\
              2018-07 Sync     17         391        21\n\
              2018-08 Sync     20         503        25\n\
              2018-09 Sync     39        1906        71\n\
              2018-10 Sync     36         728        27\n\
              2018-11 Sync      4         121        30\n"),
            (&["-F","test/emerge.10000.log","s","--duration","s","-ss","-gd"],
             "2018-02-03 Sync      1          69        69\n\
              2018-02-04 Sync      2          93        46\n\
              2018-02-05 Sync      7         188        26\n\
              2018-02-06 Sync      7         237        33\n\
              2018-02-07 Sync      7         223        31\n\
              2018-02-08 Sync      7         217        31\n\
              2018-02-09 Sync      3          92        30\n\
              2018-02-12 Sync      4          87        21\n\
              2018-02-13 Sync      2          46        23\n\
              2018-02-14 Sync      3          85        28\n\
              2018-02-15 Sync      4          77        19\n\
              2018-02-16 Sync      3          68        22\n\
              2018-02-18 Sync      1          28        28\n\
              2018-02-19 Sync      2          61        30\n\
              2018-02-20 Sync      5         120        24\n\
              2018-02-21 Sync      4          90        22\n\
              2018-02-22 Sync      2          51        25\n\
              2018-02-23 Sync      6         158        26\n\
              2018-02-24 Sync      1          23        23\n\
              2018-02-26 Sync      4          69        17\n\
              2018-02-27 Sync      8         211        26\n\
              2018-02-28 Sync      7         136        19\n\
              2018-03-01 Sync      8         569        71\n\
              2018-03-02 Sync     10         548        54\n\
              2018-03-03 Sync      2         373       186\n\
              2018-03-05 Sync      9          46         5\n\
              2018-03-06 Sync      8         183        22\n\
              2018-03-07 Sync      4         120        30\n\
              2018-03-08 Sync      8         157        19\n\
              2018-03-09 Sync      7         222        31\n\
              2018-03-12 Sync      4         121        30\n"),
        ];
        let mut tots: HashMap<&str, (u64, u64, u64, u64)> = HashMap::new();
        let to_u64 = |v: &Vec<&str>, i: usize| v.get(i).unwrap().parse::<u64>().unwrap();
        for (a, o) in t {
            // Usual output matching
            Assert::main_binary().with_args(a).stdout().is(o).unwrap();
            // Add up the "count" and "time" columns, grouped by timespan (year/month/week/day)
            for l in o.lines() {
                let cols: Vec<&str> = l.split_ascii_whitespace().collect();
                let tot = tots.entry(a.last().unwrap()).or_insert((0, 0, 0, 0));
                (*tot).0 += to_u64(&cols, 2);
                (*tot).1 += to_u64(&cols, 3);
                if cols.len() > 5 {
                    (*tot).2 += to_u64(&cols, 5);
                    (*tot).3 += to_u64(&cols, 6);
                }
            }
        }
        // Because we run the same test for each timespan, overall totals should match
        assert!(tots.iter().all(|(_, c)| c == tots.get("-gy").unwrap()),
                "Timespans should match {:?}",
                tots);
    }

    /// Test behaviour when clock goes backward between merge start and merge end. Likely to happen
    /// when you're bootstrapping an Gentoo and setting the time halfway through.
    #[test]
    fn negative_merge_time() {
        for (a, i, o) in vec![// For `log` we show an unknown time.
                 (vec!["-F", "test/emerge.negtime.log", "l", "-sms"],
                  "",
                  format!("2019-06-05 09:32:10 +01:00      1:09 Sync\n\
                           2019-06-05 12:26:54 +01:00      5:56 >>> kde-plasma/kwin-5.15.5\n\
                           2019-06-06 03:11:48 +01:00        26 >>> kde-apps/libktnef-19.04.1\n\
                           2019-06-06 03:16:01 +01:00        34 >>> net-misc/chrony-3.3\n\
                           2019-06-05 11:18:28 +01:00         ? Sync\n\
                           2019-06-05 11:21:02 +01:00         ? >>> kde-plasma/kwin-5.15.5\n\
                           2019-06-08 22:33:36 +01:00      3:10 >>> kde-plasma/kwin-5.15.5\n")),
                 // For `pred` the negative merge time is ignored.
                 (vec!["-F", "test/emerge.negtime.log", "p"],
                  "[ebuild   R   ~] kde-plasma/kwin-5.15.5\n",
                  format!("kde-plasma/kwin-5.15.5                              4:33\n\
                           Estimate for 1 ebuilds (0 unknown, 0 elapsed)       4:33 @ {}\n",
                          ts(4 * 60 + 33))),
                 // For `stats` the negative merge time is used for count but ignored for tottime/predtime.
                 (vec!["-F", "test/emerge.negtime.log", "s", "-sa"],
                  "",
                  format!("kde-apps/libktnef      1          26        26      0         0         ?\n\
                           kde-plasma/kwin        3        9:06      4:33      2         3         1\n\
                           net-misc/chrony        1          34        34      0         0         ?\n\
                           Total                  5       10:06      2:01      2         3         1\n\
                           Sync                   2        1:09      1:09\n")),]
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
