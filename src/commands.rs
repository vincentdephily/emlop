use crate::{date::*, parse::*, proces::*, table::*, *};
use std::{collections::{BTreeMap, HashMap},
          io::stdin};

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_list(args: &ArgMatches) -> Result<bool, Error> {
    let st = &Styles::from_args(args);
    let show = *args.get_one("show").unwrap();
    let hist = get_hist(args.get_one::<String>("logfile").unwrap().to_owned(),
                        value_opt(args, "from", parse_date, st.date_offset),
                        value_opt(args, "to", parse_date, st.date_offset),
                        show,
                        args.get_one::<String>("package").map(|s| s.as_str()),
                        args.get_flag("exact"))?;
    let first = *args.get_one("first").unwrap_or(&usize::MAX);
    let last = *args.get_one("last").unwrap_or(&usize::MAX);
    let stt = args.get_flag("starttime");
    let mut merges: HashMap<String, i64> = HashMap::new();
    let mut unmerges: HashMap<String, i64> = HashMap::new();
    let mut found = 0;
    let mut sync_start: Option<i64> = None;
    let mut tbl =
        Table::new(st).align(0, Align::Left).align(2, Align::Left).margin(2, " ").last(last);
    tbl.header(st.header, [&[&"Date"], &[&"Duration"], &[&"Package/Repo"]]);
    for p in hist {
        match p {
            Hist::MergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if a merge started but never finished
                merges.insert(key, ts);
            },
            Hist::MergeStop { ts, ref key, .. } => {
                found += 1;
                let started = merges.remove(key).unwrap_or(ts + 1);
                tbl.row([&[&fmt_time(if stt { started } else { ts }, st)],
                         &[&st.dur, &st.dur_t.fmt(ts - started)],
                         &[&st.merge, &p.ebuild_version()]]);
            },
            Hist::UnmergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if an unmerge started but never finished
                unmerges.insert(key, ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                found += 1;
                let started = unmerges.remove(key).unwrap_or(ts + 1);
                tbl.row([&[&fmt_time(if stt { started } else { ts }, st)],
                         &[&st.dur, &st.dur_t.fmt(ts - started)],
                         &[&st.unmerge, &p.ebuild_version()]]);
            },
            Hist::SyncStart { ts } => {
                // Some sync starts have multiple entries in old logs
                sync_start = Some(ts);
            },
            Hist::SyncStop { ts, repo } => {
                if let Some(started) = sync_start.take() {
                    found += 1;
                    tbl.row([&[&fmt_time(if stt { started } else { ts }, st)],
                             &[&st.dur, &st.dur_t.fmt(ts - started)],
                             &[&st.clr, &"Sync ", &repo]]);
                } else {
                    warn!("Sync stop without a start at {ts}")
                }
            },
        }
        if found >= first {
            break;
        }
    }
    Ok(found > 0)
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
            self.vals.insert(0, t);
            self.tot += t;
        }
    }
    /// Predict the next data point by looking at past ones
    fn pred(&self, lim: u16, avg: Average) -> i64 {
        if self.vals.is_empty() {
            return -1; // FIXME Return None
        }
        let l = self.vals.len().min(lim as usize);
        match avg {
            // Simple arithmetic mean
            Average::Arith => self.vals.iter().take(l).sum::<i64>() / l as i64,
            // Middle value (or avg of the middle two)
            Average::Median => {
                let mut s: Vec<i64> = self.vals.iter().copied().take(l).collect();
                s.sort_unstable();
                if l % 2 == 0 {
                    (s[(l / 2) - 1] + s[l / 2]) / 2
                } else {
                    s[l / 2]
                }
            },
            // Arithmically weighted arithmetic mean
            // Eg for 4 values the weights are 4,3,2,1 (most recent value first)
            Average::WeightedArith => {
                let (s, n, w) =
                    self.vals
                        .iter()
                        .take(l)
                        .fold((0, l as i64, 0), |(s, n, w), v| (s + v * n, n - 1, w + n));
                debug_assert!(n == 0);
                s / w
            },
            // Arithmically weighted median
            // Eg 3,1,2 -> 3,3,3,1,1,2 -> 1,1,2,2,3,3,3 -> 2
            Average::WeightedMedian => {
                let mut s = Vec::with_capacity((l + 1) * (l / 2 + 1));
                for (n, v) in self.vals.iter().copied().take(l).enumerate() {
                    s.resize(s.len() - n + l, v);
                }
                s.sort_unstable();
                let l = s.len();
                if l % 2 == 0 {
                    (s[(l / 2) - 1] + s[l / 2]) / 2
                } else {
                    s[l / 2]
                }
            },
        }
    }
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
pub fn cmd_stats(args: &ArgMatches) -> Result<bool, Error> {
    let st = &Styles::from_args(args);
    let show = *args.get_one("show").unwrap();
    let timespan_opt: Option<&Timespan> = args.get_one("group");
    let hist = get_hist(args.get_one::<String>("logfile").unwrap().to_owned(),
                        value_opt(args, "from", parse_date, st.date_offset),
                        value_opt(args, "to", parse_date, st.date_offset),
                        show,
                        args.get_one::<String>("package").map(|s| s.as_str()),
                        args.get_flag("exact"))?;
    let lim = *args.get_one("limit").unwrap();
    let avg = *args.get_one("avg").unwrap();
    let mut tbl = Table::new(st).align(0, Align::Left).align(1, Align::Left).margin(1, " ");
    let mut merge_start: HashMap<String, i64> = HashMap::new();
    let mut unmerge_start: HashMap<String, i64> = HashMap::new();
    let mut pkg_time: BTreeMap<String, (Times, Times)> = BTreeMap::new();
    let mut sync_start: Option<i64> = None;
    let mut sync_time: BTreeMap<String, Times> = BTreeMap::new();
    let mut nextts = 0;
    let mut curts = 0;
    for p in hist {
        if let Some(timespan) = timespan_opt {
            let t = p.ts();
            if nextts == 0 {
                nextts = timespan.next(t, st.date_offset);
                curts = t;
            } else if t > nextts {
                let group = timespan.at(curts, st.date_offset);
                cmd_stats_group(&mut tbl, st, lim, avg, show, group, &sync_time, &pkg_time)?;
                sync_time.clear();
                pkg_time.clear();
                nextts = timespan.next(t, st.date_offset);
                curts = t;
            }
        }
        match p {
            Hist::MergeStart { ts, key, .. } => {
                merge_start.insert(key, ts);
            },
            Hist::MergeStop { ts, ref key, .. } => {
                if let Some(start_ts) = merge_start.remove(key) {
                    let (times, _) = pkg_time.entry(p.ebuild().to_owned())
                                             .or_insert_with(|| (Times::new(), Times::new()));
                    times.insert(ts - start_ts);
                }
            },
            Hist::UnmergeStart { ts, key, .. } => {
                unmerge_start.insert(key, ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                if let Some(start_ts) = unmerge_start.remove(key) {
                    let (_, times) = pkg_time.entry(p.ebuild().to_owned())
                                             .or_insert_with(|| (Times::new(), Times::new()));
                    times.insert(ts - start_ts);
                }
            },
            Hist::SyncStart { ts } => {
                // Some sync starts have multiple entries in old logs
                sync_start = Some(ts);
            },
            Hist::SyncStop { ts, repo } => {
                if let Some(start_ts) = sync_start.take() {
                    let times = sync_time.entry(repo).or_insert_with(Times::new);
                    times.insert(ts - start_ts);
                } else {
                    warn!("Sync stop without a start at {ts}")
                }
            },
        }
    }
    let group =
        timespan_opt.map_or((String::new(), ""), |timespan| timespan.at(curts, st.date_offset));
    cmd_stats_group(&mut tbl, st, lim, avg, show, group, &sync_time, &pkg_time)?;
    Ok(!pkg_time.is_empty() || !sync_time.is_empty())
}

// Reducing the arg count here doesn't seem worth it, for either readability or performance
#[allow(clippy::too_many_arguments)]
fn cmd_stats_group(tbl: &mut Table<8>,
                   st: &Styles,
                   lim: u16,
                   avg: Average,
                   show: Show,
                   group: (String, &str),
                   sync_time: &BTreeMap<String, Times>,
                   pkg_time: &BTreeMap<String, (Times, Times)>)
                   -> Result<(), Error> {
    tbl.header(st.header && show.pkg | show.tot && !pkg_time.is_empty(),
               [&[&group.1],
                &[&"Package"],
                &[&"Merge count"],
                &[&"Total time"],
                &[&"Predict time"],
                &[&"Unmerge count"],
                &[&"Total time"],
                &[&"Predict time"]]);
    if show.pkg && !pkg_time.is_empty() {
        for (pkg, (merge, unmerge)) in pkg_time {
            tbl.row([&[&group.0],
                     &[&st.pkg, &pkg],
                     &[&st.cnt, &merge.count],
                     &[&st.dur, &st.dur_t.fmt(merge.tot)],
                     &[&st.dur, &st.dur_t.fmt(merge.pred(lim, avg))],
                     &[&st.cnt, &unmerge.count],
                     &[&st.dur, &st.dur_t.fmt(unmerge.tot)],
                     &[&st.dur, &st.dur_t.fmt(unmerge.pred(lim, avg))]]);
        }
    }
    if show.tot && !pkg_time.is_empty() {
        let mut merge_time = 0;
        let mut merge_count = 0;
        let mut unmerge_time = 0;
        let mut unmerge_count = 0;
        for (merge, unmerge) in pkg_time.values() {
            merge_time += merge.tot;
            merge_count += merge.count;
            unmerge_time += unmerge.tot;
            unmerge_count += unmerge.count;
        }
        tbl.row([&[&group.0],
                 &[&"Total"],
                 &[&st.cnt, &merge_count],
                 &[&st.dur, &st.dur_t.fmt(merge_time)],
                 &[&st.dur, &st.dur_t.fmt(merge_time.checked_div(merge_count).unwrap_or(-1))],
                 &[&st.cnt, &unmerge_count],
                 &[&st.dur, &st.dur_t.fmt(unmerge_time)],
                 &[&st.dur, &st.dur_t.fmt(unmerge_time.checked_div(unmerge_count).unwrap_or(-1))]]);
    }
    if show.sync && !sync_time.is_empty() {
        tbl.header(st.header,
                   [&[&group.1],
                    &[&"Repo"],
                    &[&"Sync count"],
                    &[&"Total time"],
                    &[&"Predict time"],
                    &[],
                    &[],
                    &[]]);
        for (repo, time) in sync_time {
            tbl.row([&[&group.0],
                     &[&"Sync ", &repo],
                     &[&st.cnt, &time.count],
                     &[&st.dur, &st.dur_t.fmt(time.tot)],
                     &[&st.dur, &st.dur_t.fmt(time.pred(lim, avg))],
                     &[],
                     &[],
                     &[]]);
        }
    }
    Ok(())
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(args: &ArgMatches) -> Result<bool, Error> {
    let st = &Styles::from_args(args);
    let now = epoch_now();
    let show: Show = *args.get_one("show").unwrap();
    let first = *args.get_one("first").unwrap_or(&usize::MAX);
    let last = *args.get_one("last").unwrap_or(&usize::MAX);
    let lim = *args.get_one("limit").unwrap();
    let avg = *args.get_one("avg").unwrap();
    let resume = *args.get_one("resume").unwrap();
    let mut tbl =
        Table::new(st).align(0, Align::Left).align(2, Align::Left).margin(2, " ").last(last);
    let tmpdir = args.get_one::<String>("tmpdir").unwrap();

    // Gather and print info about current merge process.
    let mut cms = std::i64::MAX;
    for i in get_all_info(Some("emerge"))? {
        cms = std::cmp::min(cms, i.start);
        if show.emerge {
            tbl.row([&[&i], &[&st.dur, &st.dur_t.fmt(now - i.start)], &[]]);
        }
    }
    if cms == std::i64::MAX
       && atty::is(atty::Stream::Stdin)
       && resume != ResumeKind::Main
       && resume != ResumeKind::Backup
    {
        tbl.row([&[&"No ongoing merge found"], &[], &[]]);
        return Ok(false);
    }

    // Parse emerge log.
    let hist = get_hist(args.get_one::<String>("logfile").unwrap().to_owned(),
                        value_opt(args, "from", parse_date, st.date_offset),
                        value_opt(args, "to", parse_date, st.date_offset),
                        Show { merge: true, ..Show::default() },
                        None,
                        false)?;
    let mut started: BTreeMap<Pkg, i64> = BTreeMap::new();
    let mut times: HashMap<String, Times> = HashMap::new();
    for p in hist {
        match p {
            Hist::MergeStart { ts, .. } => {
                started.insert(Pkg::new(p.ebuild(), p.version()), ts);
            },
            Hist::MergeStop { ts, .. } => {
                if let Some(start_ts) = started.remove(&Pkg::new(p.ebuild(), p.version())) {
                    let timevec = times.entry(p.ebuild().to_string()).or_insert_with(Times::new);
                    timevec.insert(ts - start_ts);
                }
            },
            _ => unreachable!("Should only receive Hist::{{Start,Stop}}"),
        }
    }

    // Build list of pending merges
    let pkgs: Vec<Pkg> = if atty::is(atty::Stream::Stdin) {
        // From resume data + emerge.log after current merge process start time
        let mut r = get_resume(resume);
        for p in started.iter().filter(|&(_, t)| *t > cms).map(|(p, _)| p) {
            if !r.contains(p) {
                r.push(p.clone())
            }
        }
        r
    } else {
        // From portage's stdout
        get_pretend(stdin(), "STDIN")
    };

    // Gather and print per-package and indivudual stats.
    let mut totcount = 0;
    let mut totunknown = 0;
    let mut totpredict = 0;
    let mut totelapsed = 0;
    for p in pkgs {
        totcount += 1;
        if totcount > first {
            break;
        }
        // Find the elapsed time, if any (heuristic is that emerge process started before
        // this merge finished, it's not failsafe but IMHO no worse than genlop).
        let elapsed = match started.remove(&p) {
            Some(s) if s > cms => now - s,
            _ => 0,
        };

        // Find the predicted time and adjust counters
        let pred_fmt = match times.get(p.ebuild()) {
            Some(tv) => {
                let pred = tv.pred(lim, avg);
                totpredict += pred;
                if elapsed > 0 {
                    totelapsed += elapsed;
                    totpredict -= std::cmp::min(pred, elapsed);
                }
                st.dur_t.fmt(pred)
            },
            None => {
                totunknown += 1;
                "?".into()
            },
        };

        // Done
        if show.merge {
            if elapsed > 0 {
                let stage = get_buildlog(&p, tmpdir).unwrap_or_default();
                tbl.row([&[&st.pkg, &p.ebuild_version()],
                         &[&st.dur, &pred_fmt],
                         &[&st.clr, &"- ", &st.dur, &st.dur_t.fmt(elapsed), &st.clr, &stage]]);
            } else {
                tbl.row([&[&st.pkg, &p.ebuild_version()], &[&st.dur, &pred_fmt], &[]]);
            }
        }
    }
    if totcount > 0 {
        if show.tot {
            tbl.row([&[&"Estimate for ",
                       &st.cnt,
                       &totcount,
                       &st.clr,
                       &" ebuild (",
                       &st.cnt,
                       &totunknown,
                       &st.clr,
                       &" unknown, ",
                       &st.dur,
                       &st.dur_t.fmt(totelapsed),
                       &st.clr,
                       &" elapsed)"],
                     &[&st.dur, &st.dur_t.fmt(totpredict), &st.clr],
                     &[&"@ ", &st.dur, &fmt_time(now + totpredict, st)]]);
        }
    } else {
        tbl.row([&[&"No pretended merge found"], &[], &[]]);
    }
    Ok(totcount > 0)
}

pub fn cmd_accuracy(args: &ArgMatches) -> Result<bool, Error> {
    let st = &Styles::from_args(args);
    let show: Show = *args.get_one("show").unwrap();
    let hist = get_hist(args.get_one::<String>("logfile").unwrap().to_owned(),
                        value_opt(args, "from", parse_date, st.date_offset),
                        value_opt(args, "to", parse_date, st.date_offset),
                        Show { merge: true, ..Show::default() },
                        args.get_one::<String>("package").map(|s| s.as_str()),
                        args.get_flag("exact"))?;
    let lim = *args.get_one("limit").unwrap();
    let avg = *args.get_one("avg").unwrap();
    let mut pkg_starts: HashMap<String, i64> = HashMap::new();
    let mut pkg_times: BTreeMap<String, Times> = BTreeMap::new();
    let mut pkg_errs: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut found = false;
    let mut tbl = Table::new(st).align(0, Align::Left).align(1, Align::Left);
    if show.merge {
        tbl.header(st.header,
                   [&[&"Date"], &[&"Package"], &[&"Real"], &[&"Predicted"], &[&"Error"]]);
    }
    for p in hist {
        match p {
            Hist::MergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if a merge started but never finished
                pkg_starts.insert(key, ts);
            },
            Hist::MergeStop { ts, ref key, .. } => {
                found = true;
                if let Some(start) = pkg_starts.remove(key) {
                    let times = pkg_times.entry(p.ebuild().to_owned()).or_insert_with(Times::new);
                    let real = ts - start;
                    match times.pred(lim, avg) {
                        -1 => {
                            if show.merge {
                                tbl.row([&[&fmt_time(ts, st)],
                                         &[&st.merge, &p.ebuild_version()],
                                         &[&st.dur, &st.dur_t.fmt(real)],
                                         &[],
                                         &[]])
                            }
                        },
                        pred => {
                            let err = (pred - real).abs() as f64 * 100.0 / real as f64;
                            if show.merge {
                                tbl.row([&[&fmt_time(ts, st)],
                                         &[&st.merge, &p.ebuild_version()],
                                         &[&st.dur, &st.dur_t.fmt(real)],
                                         &[&st.dur, &st.dur_t.fmt(pred)],
                                         &[&st.cnt, &format!("{err:.1}%")]])
                            }
                            let errs =
                                pkg_errs.entry(p.ebuild().to_owned()).or_insert_with(Vec::new);
                            errs.push(err);
                        },
                    }
                    times.insert(real);
                }
            },
            e => panic!("Unexpected {e:?}"),
        }
    }
    drop(tbl);
    if show.tot {
        let mut tbl = Table::new(st).align(0, Align::Left);
        tbl.header(st.header, [&[&"Package"], &[&"Error"]]);
        for (p, e) in pkg_errs {
            let avg = e.iter().sum::<f64>() / e.len() as f64;
            tbl.row([&[&st.pkg, &p], &[&st.cnt, &format!("{avg:.1}%")]]);
        }
    }
    Ok(found)
}

pub fn cmd_complete(args: &ArgMatches) -> Result<bool, Error> {
    let shell: clap_complete::Shell = *args.get_one("shell").unwrap();
    let mut cli = cli::build_cli_nocomplete();
    clap_complete::generate(shell, &mut cli, "emlop", &mut std::io::stdout());
    Ok(true)
}


#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use escargot::CargoBuild;
    use lazy_static::lazy_static;
    use std::{collections::HashMap,
              path::PathBuf,
              thread,
              time::{Duration, SystemTime, UNIX_EPOCH}};

    /// Return current unix timestamp + offset, waiting until we're close to the start of a whole
    /// second to make tests more reproducible.
    fn ts(secs: i64) -> i64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        if now.subsec_millis() > 100 {
            thread::sleep(Duration::from_millis(25));
            ts(secs)
        } else {
            now.as_secs() as i64 + secs
        }
    }

    lazy_static! {
        static ref EMLOP: PathBuf =
            CargoBuild::new().current_release().current_target().run().unwrap().path().into();
    }
    /// Return a `Command` for the main binary, making sure it is compiled first. The first call can
    /// take a while, so do a warmup call before time-sensitive tests.
    fn emlop(args: &str) -> Command {
        let mut e = Command::new(&*EMLOP);
        e.env("TZ", "UTC");
        e.args(args.split_whitespace());
        e
    }

    fn emlop_out(args: &str) -> String {
        let out = emlop(args).output().expect(&format!("could not run emlop {:?}", args));
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        String::from_utf8(out.stdout).expect("Invalid utf8")
    }

    #[test]
    fn averages() {
        use super::Times;
        use crate::Average::*;
        for (a, m, wa, wm, lim, vals) in
            [(-1, -1, -1, -1, 10, vec![]),
             (1, 1, 1, 1, 10, vec![1]),
             (12 / 2, 6, 21 / 3, 10, 10, vec![2, 10]),
             (12 / 2, 6, 14 / 3, 2, 10, vec![10, 2]),
             (15 / 3, 4, (1 + 20 + 12) / (1 + 2 + 3), 4, 10, vec![1, 10, 4]),
             (15 / 4, 2, (1 + 20 + 9 + 4) / (1 + 2 + 3 + 4), 2, 10, vec![1, 10, 3, 1]),
             (15 / 4, 2, (1 + 20 + 9 + 4) / (1 + 2 + 3 + 4), 2, 4, vec![999, 1, 10, 3, 1])]
        {
            let mut t = Times::new();
            for &v in vals.iter() {
                t.insert(v);
            }
            assert_eq!(a, t.pred(lim, Arith), "arith {lim} {vals:?}");
            assert_eq!(m, t.pred(lim, Median), "median {lim} {vals:?}");
            assert_eq!(wa, t.pred(lim, WeightedArith), "weighted arith {lim} {vals:?}");
            assert_eq!(wm, t.pred(lim, WeightedMedian), "weighted median {lim} {vals:?}");
        }
    }

    #[test]
    fn log() {
        let t: Vec<(&str, &str)> = vec![
            // Basic test
            ("-F test/emerge.10000.log l client",
             "2018-02-04 04:55:19    35:46 >>> mail-client/thunderbird-52.6.0\n\
              2018-02-04 05:42:48    47:29 >>> www-client/firefox-58.0.1\n\
              2018-02-09 11:04:59    47:58 >>> mail-client/thunderbird-52.6.0-r1\n\
              2018-02-12 10:14:11       31 >>> kde-frameworks/kxmlrpcclient-5.43.0\n\
              2018-02-16 04:41:39  6:03:14 >>> www-client/chromium-64.0.3282.140\n\
              2018-02-19 17:35:41  7:56:03 >>> www-client/chromium-64.0.3282.167\n\
              2018-02-22 13:32:53       44 >>> www-client/links-2.14-r1\n\
              2018-02-28 09:14:37     6:02 >>> www-client/falkon-3.0.0\n\
              2018-03-06 04:19:52  7:42:07 >>> www-client/chromium-64.0.3282.186\n\
              2018-03-12 10:35:22       14 >>> x11-apps/xlsclients-1.1.4\n\
              2018-03-12 11:03:53       16 >>> kde-frameworks/kxmlrpcclient-5.44.0\n"),
            // Check output when duration isn't known
            ("-F test/emerge.10000.log l -s m mlt -e --from 2018-02-18T12:37:00",
             "2018-02-18 12:37:09   ? >>> media-libs/mlt-6.4.1-r6\n\
              2018-02-27 15:10:05  43 >>> media-libs/mlt-6.4.1-r6\n\
              2018-02-27 16:48:40  39 >>> media-libs/mlt-6.4.1-r6\n"),
            // Check output of sync events
            ("-F test/emerge.10000.log l -ss --from 2018-03-07T10:42:00 --to 2018-03-07T14:00:00",
             "2018-03-07 11:37:05  38 Sync gentoo\n\
              2018-03-07 13:56:09  40 Sync gentoo\n"),
            ("-F test/emerge.sync.log l -ss",
             "2007-04-06 04:43:38    26:02 Sync gentoo-portage\n\
              2007-04-09 21:30:01    19:20 Sync gentoo-portage\n\
              2007-04-16 21:52:59    59:53 Sync gentoo-portage\n\
              2007-04-19 19:05:59    31:53 Sync gentoo-portage\n\
              2007-05-09 02:14:35  2:15:34 Sync gentoo-portage\n\
              2016-01-08 21:17:59       38 Sync gentoo\n\
              2016-01-10 23:31:57       49 Sync gentoo\n\
              2017-02-03 21:14:50       53 Sync gentoo\n\
              2017-02-06 21:06:18       55 Sync gentoo\n\
              2017-02-07 21:00:51     3:01 Sync gentoo\n\
              2017-02-09 22:06:12    13:22 Sync gentoo\n\
              2017-02-12 15:59:38       39 Sync gentoo\n\
              2017-02-12 20:43:23     1:48 Sync gentoo\n\
              2017-02-13 21:11:46     8:12 Sync gentoo\n\
              2017-03-11 20:59:57     8:26 Sync gentoo\n\
              2017-03-16 21:17:59       27 Sync gentoo\n\
              2017-03-22 22:13:50  1:00:31 Sync gentoo\n\
              2017-03-31 20:07:49     1:40 Sync gentoo\n\
              2017-04-03 20:10:55       24 Sync gentoo\n\
              2017-04-04 20:11:08       19 Sync gentoo\n\
              2020-06-16 08:53:33        6 Sync gentoo\n\
              2020-06-16 08:53:34        1 Sync moltonel\n\
              2020-06-16 15:40:57       14 Sync gentoo\n\
              2020-06-16 15:41:07       10 Sync moltonel\n\
              2020-06-16 15:54:16        6 Sync gentoo\n\
              2020-06-16 15:54:21        5 Sync steam-overlay\n\
              2020-06-16 15:54:24        3 Sync moltonel\n\
              2020-06-16 16:21:41        3 Sync gentoo\n\
              2020-06-16 16:21:42        1 Sync steam-overlay\n\
              2020-06-16 16:21:44        2 Sync moltonel\n\
              2020-06-16 20:58:00        7 Sync moltonel\n\
              2020-06-16 21:36:46        4 Sync gentoo\n\
              2020-06-16 21:36:47        1 Sync steam-overlay\n\
              2020-06-16 21:36:48        1 Sync moltonel\n\
              2020-06-17 20:24:00       30 Sync gentoo\n\
              2020-06-17 20:24:02        2 Sync steam-overlay\n\
              2020-06-17 20:24:03        1 Sync moltonel\n\
              2020-06-18 16:21:54        6 Sync gentoo\n\
              2020-06-18 16:21:55        1 Sync steam-overlay\n\
              2020-06-18 16:21:56        1 Sync moltonel\n"),
            // Check output of all events
            ("-F test/emerge.10000.log l --show a --from 2018-03-07T10:42:00 --to 2018-03-07T14:00:00",
             "2018-03-07 10:43:10    14 >>> sys-apps/the_silver_searcher-2.0.0\n\
              2018-03-07 11:37:05    38 Sync gentoo\n\
              2018-03-07 12:49:09     2 <<< sys-apps/util-linux-2.30.2\n\
              2018-03-07 12:49:13  1:01 >>> sys-apps/util-linux-2.30.2-r1\n\
              2018-03-07 13:56:09    40 Sync gentoo\n\
              2018-03-07 13:59:38     2 <<< dev-libs/nspr-4.17\n\
              2018-03-07 13:59:41    24 >>> dev-libs/nspr-4.18\n")
        ];
        for (a, o) in t {
            emlop(a).assert().stdout(o);
        }
    }

    #[test]
    fn starttime() {
        let o1 = emlop_out("-F test/emerge.10000.log l --dat=unix --dur=s");
        let o2 = emlop_out("-F test/emerge.10000.log l --dat=unix --dur=s --starttime");
        let mut lines = 0;
        for (l1, l2) in o1.lines().zip(o2.lines()) {
            lines += 1;
            let mut w1 = l1.split_ascii_whitespace();
            let mut w2 = l2.split_ascii_whitespace();
            let t1 = dbg!(w1.next()).expect("missing t1").parse::<u64>().expect("bad int t1");
            let d1 = dbg!(w1.next()).expect("missing d1");
            let t2 = dbg!(w2.next()).expect("missing t2").parse::<u64>().expect("bad int t2");
            let d2 = dbg!(w2.next()).expect("missing d2");
            assert!(d1 == d2);
            if d1 != "?" {
                assert!(t2 + d1.parse::<u64>().expect("bad int d1") == t1);
            }
        }
        assert!(lines > 500);
    }

    #[test]
    fn timezone() {
        let t: Vec<(&str, &str)> = vec![
            // UTC
            ("UTC",
             "2021-03-26 17:07:08 +00:00    20 >>> dev-libs/libksba-1.5.0\n\
              2021-03-26 17:08:20 +00:00  1:12 >>> sys-boot/grub-2.06_rc1\n\
              2021-03-29 10:57:14 +00:00    12 >>> sys-apps/install-xattr-0.8\n\
              2021-03-29 10:57:45 +00:00    31 >>> sys-devel/m4-1.4.18-r2\n"),
            // Moscow (east)
            ("Europe/Moscow",
             "2021-03-26 20:07:08 +03:00    20 >>> dev-libs/libksba-1.5.0\n\
              2021-03-26 20:08:20 +03:00  1:12 >>> sys-boot/grub-2.06_rc1\n\
              2021-03-29 13:57:14 +03:00    12 >>> sys-apps/install-xattr-0.8\n\
              2021-03-29 13:57:45 +03:00    31 >>> sys-devel/m4-1.4.18-r2\n"),
            // Marquesas island (west, non-whole)
            ("Pacific/Marquesas",
             "2021-03-26 07:37:08 -09:30    20 >>> dev-libs/libksba-1.5.0\n\
              2021-03-26 07:38:20 -09:30  1:12 >>> sys-boot/grub-2.06_rc1\n\
              2021-03-29 01:27:14 -09:30    12 >>> sys-apps/install-xattr-0.8\n\
              2021-03-29 01:27:45 -09:30    31 >>> sys-devel/m4-1.4.18-r2\n"),
            // Dublin (affected by DST)
            // FIXME: Hanling this properly will remain impossible until UtcOffset::local_offset_at
            //        functionality is available after thread start (see
            //        https://github.com/time-rs/time/issues/380). Until then, emlop's behavior is
            //        to display all dates with the same offset (the curent one detected at program
            //        start) even though it should be different at different dates. Not adding a
            //        unitest for this, as it would need to be updated twice a year.
            //(&["-F", "test/emerge.dst.log", "l"],
            // "Europe/Dublin",
            // "2021-03-26 17:07:08 +00:00    20 >>> dev-libs/libksba-1.5.0\n\
            //  2021-03-26 17:08:20 +00:00  1:12 >>> sys-boot/grub-2.06_rc1\n\
            //  2021-03-29 11:57:14 +01:00    12 >>> sys-apps/install-xattr-0.8\n\
            //  2021-03-29 11:57:45 +01:00    31 >>> sys-devel/m4-1.4.18-r2\n"),
        ];
        for (t, o) in t {
            emlop("-F test/emerge.dst.log l --date dto").env("TZ", t).assert().stdout(o);
        }
    }

    /// Check Basic 'emlop p`. Not a hugely useful test, but it's something.
    ///
    /// Ignored by default: depends on there being no currently running emerge.
    #[ignore]
    #[test]
    fn predict_tty() {
        emlop("p -F test/emerge.10000.log").assert().code(1).stdout("No pretended merge found\n");
    }

    /// Ignored by default: depends on there being no currently running emerge.
    #[ignore]
    #[test]
    fn predict_emerge_p() {
        let _cache_cargo_build = emlop("");
        let t = vec![// Check garbage input
                     ("blah blah\n", format!("No pretended merge found\n"), 1),
                     // Check all-unknowns
                     ("[ebuild   R   ~] dev-lang/unknown-1.42\n",
                      format!("dev-lang/unknown-1.42                         ? \n\
                               Estimate for 1 ebuild (1 unknown, 0 elapsed)  0 @ {}\n",
                              ts(0)),
                      0),
                     // Check that unknown ebuild don't wreck alignment. Remember that times are {:>9}
                     ("[ebuild   R   ~] dev-qt/qtcore-5.9.4-r2\n\
                       [ebuild   R   ~] dev-lang/unknown-1.42\n\
                       [ebuild   R   ~] dev-qt/qtgui-5.9.4-r3\n",
                      format!("dev-qt/qtcore-5.9.4-r2                        3:45 \n\
                               dev-lang/unknown-1.42                            ? \n\
                               dev-qt/qtgui-5.9.4-r3                         4:24 \n\
                               Estimate for 3 ebuild (1 unknown, 0 elapsed)  8:09 @ {}\n",
                              ts(8 * 60 + 9)),
                      0),];
        for (i, o, e) in t {
            emlop("-F test/emerge.10000.log p --date unix").write_stdin(i)
                                                           .assert()
                                                           .code(e)
                                                           .stdout(o);
        }
    }

    #[test]
    fn stats() {
        let t: Vec<(&str, &str, i32)> = vec![
            ("-F test/emerge.10000.log s client",
             "kde-frameworks/kxmlrpcclient  2        47       23  2   4  2\n\
              mail-client/thunderbird       2   1:23:44    41:52  2   6  3\n\
              www-client/chromium           3  21:41:24  7:42:07  3  12  3\n\
              www-client/falkon             1      6:02     6:02  0   0  ?\n\
              www-client/firefox            1     47:29    47:29  1   3  3\n\
              www-client/links              1        44       44  1   1  1\n\
              x11-apps/xlsclients           1        14       14  1   1  1\n",
             0),
            ("-F test/emerge.sync.log s -ss",
             "Sync gentoo          22  1:43:13     10\n\
              Sync gentoo-portage   5  4:32:42  31:53\n\
              Sync moltonel         8       26      1\n\
              Sync steam-overlay    5       10      1\n",
             0),
            ("-F test/emerge.sync.log s -ss gentoo",
             "Sync gentoo          22  1:43:13     10\n\
              Sync gentoo-portage   5  4:32:42  31:53\n",
             0),
            ("-F test/emerge.10000.log s client -sst",
             "Total  11  24:00:24  2:10:56  10  27  2\n",
             0),
            ("-F test/emerge.10000.log s client -sa",
             "kde-frameworks/kxmlrpcclient   2        47       23   2   4  2\n\
              mail-client/thunderbird        2   1:23:44    41:52   2   6  3\n\
              www-client/chromium            3  21:41:24  7:42:07   3  12  3\n\
              www-client/falkon              1      6:02     6:02   0   0  ?\n\
              www-client/firefox             1     47:29    47:29   1   3  3\n\
              www-client/links               1        44       44   1   1  1\n\
              x11-apps/xlsclients            1        14       14   1   1  1\n\
              Total                         11  24:00:24  2:10:56  10  27  2\n",
             0),
            ("-F test/emerge.10000.log s gentoo-sources --avg arith",
             "sys-kernel/gentoo-sources  10  15:04  1:30  11  3:20  16\n",
             0),
            ("-F test/emerge.10000.log s gentoo-sources --avg median",
             "sys-kernel/gentoo-sources  10  15:04  1:21  11  3:20  13\n",
             0),
            ("-F test/emerge.10000.log s gentoo-sources --avg weighted-arith",
             "sys-kernel/gentoo-sources  10  15:04  1:31  11  3:20  17\n",
             0),
            ("-F test/emerge.10000.log s gentoo-sources --avg weighted-median",
             "sys-kernel/gentoo-sources  10  15:04  1:22  11  3:20  15\n",
             0),
            ("-F test/emerge.10000.log s --from 2018-02-03T23:11:47 --to 2018-02-04 notfound -sa",
             "",
             1),
        ];
        for (a, o, e) in t {
            emlop(a).assert().code(e).stdout(o);
        }
    }

    /// Test grouped stats. In addition to the usual check that the actual output matches the
    /// expected one, we check that the expected outputs are consistent (y/m/w/d totals are the
    /// same, and avg*count==tot).
    #[test]
    fn stats_grouped() {
        let t: Vec<(&str, &str)> = vec![
            ("-F test/emerge.10000.log s --duration s -sp gentoo-sources -gy",
             "2018 sys-kernel/gentoo-sources  10  904  81  11  200  13\n"),
            ("-F test/emerge.10000.log s --duration s -sp gentoo-sources -gm",
             "2018-02 sys-kernel/gentoo-sources  8  702   80  8  149  13\n\
              2018-03 sys-kernel/gentoo-sources  2  202  101  3   51  15\n"),
            ("-F test/emerge.10000.log s --duration s -sp gentoo-sources -gw",
             "2018-05 sys-kernel/gentoo-sources  1   81   81  0   0   ?\n\
              2018-06 sys-kernel/gentoo-sources  2  192   96  3  66  14\n\
              2018-07 sys-kernel/gentoo-sources  2  198   99  0   0   ?\n\
              2018-08 sys-kernel/gentoo-sources  1   77   77  3  37  12\n\
              2018-09 sys-kernel/gentoo-sources  3  236   79  3  61  22\n\
              2018-10 sys-kernel/gentoo-sources  0    0    ?  1  23  23\n\
              2018-11 sys-kernel/gentoo-sources  1  120  120  1  13  13\n"),
            ("-F test/emerge.10000.log s --duration s -sp gentoo-sources -gd",
             "2018-02-04 sys-kernel/gentoo-sources  1   81   81  0   0   ?\n\
              2018-02-05 sys-kernel/gentoo-sources  1   95   95  0   0   ?\n\
              2018-02-06 sys-kernel/gentoo-sources  0    0    ?  3  66  14\n\
              2018-02-08 sys-kernel/gentoo-sources  1   97   97  0   0   ?\n\
              2018-02-12 sys-kernel/gentoo-sources  1   80   80  0   0   ?\n\
              2018-02-18 sys-kernel/gentoo-sources  1  118  118  0   0   ?\n\
              2018-02-22 sys-kernel/gentoo-sources  0    0    ?  3  37  12\n\
              2018-02-23 sys-kernel/gentoo-sources  1   77   77  0   0   ?\n\
              2018-02-26 sys-kernel/gentoo-sources  1   79   79  0   0   ?\n\
              2018-02-27 sys-kernel/gentoo-sources  0    0    ?  2  46  23\n\
              2018-02-28 sys-kernel/gentoo-sources  1   75   75  0   0   ?\n\
              2018-03-01 sys-kernel/gentoo-sources  1   82   82  1  15  15\n\
              2018-03-05 sys-kernel/gentoo-sources  0    0    ?  1  23  23\n\
              2018-03-12 sys-kernel/gentoo-sources  1  120  120  1  13  13\n"),
            ("-F test/emerge.10000.log s --duration s -st -gy",
             "2018 Total  831  216426  260  832  2311  2\n"),
            ("-F test/emerge.10000.log s --duration s -st -gm",
             "2018-02 Total  533  158312  297  529  1497  2\n\
              2018-03 Total  298   58114  195  303   814  2\n"),
            ("-F test/emerge.10000.log s --duration s -st -gw",
             "2018-05 Total   63  33577  532   60  132  2\n\
              2018-06 Total   74  10070  136   68  225  3\n\
              2018-07 Total  281  58604  208  258  709  2\n\
              2018-08 Total   65  51276  788   69  197  2\n\
              2018-09 Total   71  14737  207   95  316  3\n\
              2018-10 Total  182  43782  240  187  519  2\n\
              2018-11 Total   95   4380   46   95  213  2\n"),
            ("-F test/emerge.10000.log s --duration s -st -gd",
             "2018-02-03 Total   32   2741     85   32   70  2\n\
              2018-02-04 Total   31  30836    994   28   62  2\n\
              2018-02-05 Total    4    158     39    3    5  1\n\
              2018-02-06 Total   44   4288     97   44  174  3\n\
              2018-02-07 Total   15    857     57   13   28  2\n\
              2018-02-08 Total    5    983    196    4    8  2\n\
              2018-02-09 Total    6   3784    630    4   10  2\n\
              2018-02-12 Total  208  29239    140  206  587  2\n\
              2018-02-13 Total    1     19     19    0    0  ?\n\
              2018-02-14 Total   44   4795    108   44   92  2\n\
              2018-02-15 Total    3    137     45    3    6  2\n\
              2018-02-16 Total   21  23914   1138    3   14  4\n\
              2018-02-18 Total    4    500    125    2   10  5\n\
              2018-02-19 Total    2  28977  14488    2    6  3\n\
              2018-02-20 Total    2    488    244    1    2  2\n\
              2018-02-21 Total   37   5522    149   36   93  2\n\
              2018-02-22 Total   16  15396    962   23   82  3\n\
              2018-02-23 Total    6    854    142    5   11  2\n\
              2018-02-24 Total    2     39     19    2    3  1\n\
              2018-02-26 Total   10   2730    273    9   18  2\n\
              2018-02-27 Total   35   1403     40   49  175  3\n\
              2018-02-28 Total    5    652    130   16   41  2\n\
              2018-03-01 Total   13   9355    719   13   40  3\n\
              2018-03-02 Total    5    510    102    5   37  7\n\
              2018-03-03 Total    3     87     29    3    5  1\n\
              2018-03-05 Total    9    168     18   21   84  4\n\
              2018-03-06 Total    3  27746   9248    1    3  3\n\
              2018-03-07 Total   46   2969     64   43   90  2\n\
              2018-03-08 Total   74   5441     73   73  202  2\n\
              2018-03-09 Total   50   7458    149   49  140  2\n\
              2018-03-12 Total   95   4380     46   95  213  2\n"),
            ("-F test/emerge.10000.log s --duration s -ss -gy",
             "2018 Sync gentoo  150  4747  28\n"),
            ("-F test/emerge.10000.log s --duration s -ss -gm",
             "2018-02 Sync gentoo  90  2411  15\n\
              2018-03 Sync gentoo  60  2336  28\n"),
            ("-F test/emerge.10000.log s --duration s -ss -gw",
             "2018-05 Sync gentoo   3   160  56\n\
              2018-06 Sync gentoo  31   951  27\n\
              2018-07 Sync gentoo  17   388  19\n\
              2018-08 Sync gentoo  20   500  23\n\
              2018-09 Sync gentoo  39  1899  49\n\
              2018-10 Sync gentoo  36   728  21\n\
              2018-11 Sync gentoo   4   121  32\n"),
            ("-F test/emerge.10000.log s --duration s -ss -gd",
             "2018-02-03 Sync gentoo   1   68   68\n\
              2018-02-04 Sync gentoo   2   92   46\n\
              2018-02-05 Sync gentoo   7  186   32\n\
              2018-02-06 Sync gentoo   7  237   31\n\
              2018-02-07 Sync gentoo   7  221   32\n\
              2018-02-08 Sync gentoo   7  215   21\n\
              2018-02-09 Sync gentoo   3   92   29\n\
              2018-02-12 Sync gentoo   4   87   22\n\
              2018-02-13 Sync gentoo   2   45   22\n\
              2018-02-14 Sync gentoo   3   85   23\n\
              2018-02-15 Sync gentoo   4   76   18\n\
              2018-02-16 Sync gentoo   3   67   20\n\
              2018-02-18 Sync gentoo   1   28   28\n\
              2018-02-19 Sync gentoo   2   61   30\n\
              2018-02-20 Sync gentoo   5  119   22\n\
              2018-02-21 Sync gentoo   4   89   21\n\
              2018-02-22 Sync gentoo   2   51   25\n\
              2018-02-23 Sync gentoo   6  157   24\n\
              2018-02-24 Sync gentoo   1   23   23\n\
              2018-02-26 Sync gentoo   4   69   17\n\
              2018-02-27 Sync gentoo   8  208   20\n\
              2018-02-28 Sync gentoo   7  135   16\n\
              2018-03-01 Sync gentoo   8  568   30\n\
              2018-03-02 Sync gentoo  10  547   49\n\
              2018-03-03 Sync gentoo   2  372  186\n\
              2018-03-05 Sync gentoo   9   46    1\n\
              2018-03-06 Sync gentoo   8  183   22\n\
              2018-03-07 Sync gentoo   4  120   34\n\
              2018-03-08 Sync gentoo   8  157   20\n\
              2018-03-09 Sync gentoo   7  222   31\n\
              2018-03-12 Sync gentoo   4  121   32\n"),
        ];
        let mut tots: HashMap<&str, (u64, u64, u64, u64)> = HashMap::new();
        let to_u64 = |v: &Vec<&str>, i: usize| v.get(i).unwrap().parse::<u64>().unwrap();
        for (a, o) in t {
            // Usual output matching
            emlop(a).assert().success().stdout(o);
            // Add up the "count" and "time" columns, grouped by timespan (year/month/week/day)
            for l in o.lines() {
                let cols: Vec<&str> = l.split_ascii_whitespace().collect();
                let tot = tots.entry(a.split_whitespace().last().unwrap()).or_insert((0, 0, 0, 0));
                match cols.len() {
                    // Sync
                    6 => {
                        (*tot).0 += to_u64(&cols, 3);
                        (*tot).1 += to_u64(&cols, 4);
                    },
                    // merge
                    8 => {
                        (*tot).0 += to_u64(&cols, 2);
                        (*tot).1 += to_u64(&cols, 3);
                        (*tot).2 += to_u64(&cols, 5);
                        (*tot).3 += to_u64(&cols, 6);
                    },
                    _ => panic!("Unexpected col count {l}"),
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
        let _cache_cargo_build = emlop("");
        for (a, o) in
            vec![// For `log` we show an unknown time.
                 ("-F test/emerge.negtime.log l -sms",
                  format!("2019-06-05 08:32:10  1:06 Sync gentoo\n\
                           2019-06-05 11:26:54  5:56 >>> kde-plasma/kwin-5.15.5\n\
                           2019-06-06 02:11:48    26 >>> kde-apps/libktnef-19.04.1\n\
                           2019-06-06 02:16:01    34 >>> net-misc/chrony-3.3\n\
                           2019-06-05 10:18:28     ? Sync gentoo\n\
                           2019-06-05 10:21:02     ? >>> kde-plasma/kwin-5.15.5\n\
                           2019-06-08 21:33:36  3:10 >>> kde-plasma/kwin-5.15.5\n")),
                 // For `stats` the negative merge time is used for count but ignored for tottime/predtime.
                 ("-F test/emerge.negtime.log s -sa",
                  format!("kde-apps/libktnef  1     26    26  0  0  ?\n\
                           kde-plasma/kwin    3   9:06  4:33  2  3  1\n\
                           net-misc/chrony    1     34    34  0  0  ?\n\
                           Total              5  10:06  2:01  2  3  1\n\
                           Sync gentoo        2   1:06  1:06         \n")),]
        {
            emlop(a).assert().success().stdout(o);
        }
    }

    /// Same as negative_merge_time() but for predict command.
    /// For `pred` the negative merge time is ignored.
    ///
    /// Ignored by default: depends on there being no currently running emerge.
    #[ignore]
    #[test]
    fn negative_merge_time_pred() {
        let _cache_cargo_build = emlop("");
        let a = "-F test/emerge.negtime.log p --date unix";
        let i = "[ebuild   R   ~] kde-plasma/kwin-5.15.5\n";
        let o = format!("kde-plasma/kwin-5.15.5                        4:33 \n\
                         Estimate for 1 ebuild (0 unknown, 0 elapsed)  4:33 @ {}\n",
                        ts(4 * 60 + 33));
        emlop(a).write_stdin(i).assert().success().stdout(o);
    }

    #[test]
    fn exit_status() {
        // 0: no problem
        // 1: command ran properly but didn't find anything
        // 2: user or program error
        let t: Vec<(&str, i32)> = vec![// Help, version, badarg (clap)
                                       ("-h", 0),
                                       ("-V", 0),
                                       ("l -h", 0),
                                       ("", 2),
                                       ("s --foo", 2),
                                       ("badcmd", 2),
                                       ("--utc", 2),
                                       // Bad arguments (emlop)
                                       ("l --logfile notfound", 2),
                                       ("s --logfile notfound", 2),
                                       ("p --logfile notfound", 2),
                                       ("l bad regex [a-z", 2),
                                       ("s bad regex [a-z", 2),
                                       ("p bad regex [a-z", 2),
                                       // Normal behaviour
                                       ("-F test/emerge.10000.log p", 1),
                                       ("-F test/emerge.10000.log l", 0),
                                       ("-F test/emerge.10000.log l -sm", 0),
                                       ("-F test/emerge.10000.log l -e icu", 0),
                                       ("-F test/emerge.10000.log l -e unknown", 1),
                                       ("-F test/emerge.10000.log l --from 2018-09-28", 1),
                                       ("-F test/emerge.10000.log l -sm --from 2018-09-28", 1),
                                       ("-F test/emerge.10000.log s", 0),
                                       ("-F test/emerge.10000.log s -e icu", 0),
                                       ("-F test/emerge.10000.log s -e unknown", 1),];
        for (a, e) in t {
            emlop(a).assert().code(e);
        }
    }
}
