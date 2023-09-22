use crate::{datetime::*, parse::*, proces::*, table::*, *};
use std::{collections::{BTreeMap, HashMap},
          io::stdin};

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_list(args: &ArgMatches) -> Result<bool, Error> {
    let st = &Styles::from_args(args);
    let show = *args.get_one("show").unwrap();
    let hist = get_hist(args.get_one::<String>("logfile").unwrap().to_owned(),
                        get_parse(args, "from", parse_date, st.date_offset)?,
                        get_parse(args, "to", parse_date, st.date_offset)?,
                        show,
                        args.get_many::<String>("search").unwrap_or_default().cloned().collect(),
                        args.get_flag("exact"))?;
    let first = *args.get_one("first").unwrap_or(&usize::MAX);
    let last = *args.get_one("last").unwrap_or(&usize::MAX);
    let stt = args.get_flag("starttime");
    let mut merges: HashMap<String, i64> = HashMap::new();
    let mut unmerges: HashMap<String, i64> = HashMap::new();
    let mut found = 0;
    let mut sync_start: Option<i64> = None;
    let mut tbl = Table::new(st).align_left(0).align_left(2).margin(2, " ").last(last);
    tbl.header(["Date", "Duration", "Package/Repo"]);
    for p in hist {
        match p {
            Hist::MergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if a merge started but never finished
                merges.insert(key, ts);
            },
            Hist::MergeStop { ts, ref key, .. } => {
                found += 1;
                let started = merges.remove(key).unwrap_or(ts + 1);
                tbl.row([&[&FmtDate(if stt { started } else { ts })],
                         &[&FmtDur(ts - started)],
                         &[&st.merge, &p.ebuild_version()]]);
            },
            Hist::UnmergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if an unmerge started but never finished
                unmerges.insert(key, ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                found += 1;
                let started = unmerges.remove(key).unwrap_or(ts + 1);
                tbl.row([&[&FmtDate(if stt { started } else { ts })],
                         &[&FmtDur(ts - started)],
                         &[&st.unmerge, &p.ebuild_version()]]);
            },
            Hist::SyncStart { ts } => {
                // Some sync starts have multiple entries in old logs
                sync_start = Some(ts);
            },
            Hist::SyncStop { ts, repo } => {
                if let Some(started) = sync_start.take() {
                    found += 1;
                    tbl.row([&[&FmtDate(if stt { started } else { ts })],
                             &[&FmtDur(ts - started)],
                             &[&st.clr, &"Sync ", &repo]]);
                } else {
                    warn!("Sync stop without a start at {ts}");
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
                        get_parse(args, "from", parse_date, st.date_offset)?,
                        get_parse(args, "to", parse_date, st.date_offset)?,
                        show,
                        args.get_many::<String>("search").unwrap_or_default().cloned().collect(),
                        args.get_flag("exact"))?;
    let lim = *args.get_one("limit").unwrap();
    let avg = *args.get_one("avg").unwrap();
    let tsname = timespan_opt.map_or("", |timespan| timespan.name());
    let mut tbls = Table::new(st).align_left(0).align_left(1).margin(1, " ");
    tbls.header([tsname, "Repo", "Sync count", "Total time", "Predict time"]);
    let mut tblp = Table::new(st).align_left(0).align_left(1).margin(1, " ");
    tblp.header([tsname,
                 "Package",
                 "Merge count",
                 "Total time",
                 "Predict time",
                 "Unmerge count",
                 "Total time",
                 "Predict time"]);
    let mut tblt = Table::new(st).align_left(0).margin(1, " ");
    tblt.header([tsname,
                 "Merge count",
                 "Total time",
                 "Predict time",
                 "Unmerge count",
                 "Total time",
                 "Predict time"]);
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
                cmd_stats_group(&mut tbls, &mut tblp, &mut tblt, st, lim, avg, show, group,
                                &sync_time, &pkg_time);
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
    let group = timespan_opt.map(|timespan| timespan.at(curts, st.date_offset)).unwrap_or_default();
    cmd_stats_group(&mut tbls, &mut tblp, &mut tblt, st, lim, avg, show, group, &sync_time,
                    &pkg_time);
    // Controlled drop to ensure table order and insert blank lines
    let (es, ep, et) = (!tbls.is_empty(), !tblp.is_empty(), !tblt.is_empty());
    drop(tbls);
    if es && ep {
        println!();
    }
    drop(tblp);
    if (es || ep) && et {
        println!();
    }
    drop(tblt);
    Ok(!pkg_time.is_empty() || !sync_time.is_empty())
}

// Reducing the arg count here doesn't seem worth it, for either readability or performance
#[allow(clippy::too_many_arguments)]
fn cmd_stats_group(tbls: &mut Table<5>,
                   tblp: &mut Table<8>,
                   tblt: &mut Table<7>,
                   st: &Styles,
                   lim: u16,
                   avg: Average,
                   show: Show,
                   group: String,
                   sync_time: &BTreeMap<String, Times>,
                   pkg_time: &BTreeMap<String, (Times, Times)>) {
    // Syncs
    if show.sync && !sync_time.is_empty() {
        for (repo, time) in sync_time {
            tbls.row([&[&group],
                      &[repo],
                      &[&st.cnt, &time.count],
                      &[&FmtDur(time.tot)],
                      &[&FmtDur(time.pred(lim, avg))]]);
        }
    }
    // Packages
    if show.pkg && !pkg_time.is_empty() {
        for (pkg, (merge, unmerge)) in pkg_time {
            tblp.row([&[&group],
                      &[&st.pkg, pkg],
                      &[&st.cnt, &merge.count],
                      &[&FmtDur(merge.tot)],
                      &[&FmtDur(merge.pred(lim, avg))],
                      &[&st.cnt, &unmerge.count],
                      &[&FmtDur(unmerge.tot)],
                      &[&FmtDur(unmerge.pred(lim, avg))]]);
        }
    }
    // Totals
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
        tblt.row([&[&group],
                  &[&st.cnt, &merge_count],
                  &[&FmtDur(merge_time)],
                  &[&FmtDur(merge_time.checked_div(merge_count).unwrap_or(-1))],
                  &[&st.cnt, &unmerge_count],
                  &[&FmtDur(unmerge_time)],
                  &[&FmtDur(unmerge_time.checked_div(unmerge_count).unwrap_or(-1))]]);
    }
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(args: &ArgMatches) -> Result<bool, Error> {
    let st = &Styles::from_args(args);
    let now = epoch_now();
    let show: Show = *args.get_one("show").unwrap();
    let first = *args.get_one("first").unwrap_or(&usize::MAX);
    let last = match args.get_one("last") {
        Some(&n) if show.tot => n + 1,
        Some(&n) => n,
        None => usize::MAX,
    };
    let lim = *args.get_one("limit").unwrap();
    let avg = *args.get_one("avg").unwrap();
    let resume = *args.get_one("resume").unwrap();
    let mut tbl = Table::new(st).align_left(0).align_left(2).margin(2, " ").last(last);
    let tmpdirs = args.get_many::<String>("tmpdir").unwrap().cloned().collect();

    // Gather and print info about current merge process.
    let mut cms = std::i64::MAX;
    for i in get_all_info(Some("emerge")) {
        cms = std::cmp::min(cms, i.start);
        if show.emerge {
            tbl.row([&[&i], &[&FmtDur(now - i.start)], &[]]);
        }
    }
    if cms == std::i64::MAX
       && std::io::stdin().is_terminal()
       && resume != ResumeKind::Main
       && resume != ResumeKind::Backup
    {
        tbl.row([&[&"No ongoing merge found"], &[], &[]]);
        return Ok(false);
    }

    // Parse emerge log.
    let hist = get_hist(args.get_one::<String>("logfile").unwrap().to_owned(),
                        get_parse(args, "from", parse_date, st.date_offset)?,
                        get_parse(args, "to", parse_date, st.date_offset)?,
                        Show { merge: true, ..Show::default() },
                        vec![],
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
    let pkgs: Vec<Pkg> = if std::io::stdin().is_terminal() {
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
        // Find the elapsed time, if any (heuristic is that emerge process started before
        // this merge finished, it's not failsafe but IMHO no worse than genlop).
        let elapsed = match started.remove(&p) {
            Some(s) if s > cms => now - s,
            _ => 0,
        };

        // Find the predicted time and adjust counters
        let pred = match times.get(p.ebuild()) {
            Some(tv) => {
                let pred = tv.pred(lim, avg);
                totpredict += pred;
                if elapsed > 0 {
                    totelapsed += elapsed;
                    totpredict -= std::cmp::min(pred, elapsed);
                }
                pred
            },
            None => {
                totunknown += 1;
                -1
            },
        };

        // Done
        if show.merge && totcount <= first {
            if elapsed > 0 {
                let stage = get_buildlog(&p, &tmpdirs).unwrap_or_default();
                tbl.row([&[&st.pkg, &p.ebuild_version()],
                         &[&FmtDur(pred)],
                         &[&st.clr, &"- ", &FmtDur(elapsed), &st.clr, &stage]]);
            } else {
                tbl.row([&[&st.pkg, &p.ebuild_version()], &[&FmtDur(pred)], &[]]);
            }
        }
    }
    if totcount > 0 {
        if show.tot {
            let mut s: Vec<&dyn Disp> = vec![&"Estimate for ",
                                             &st.cnt,
                                             &totcount,
                                             &st.clr,
                                             if totcount > 1 { &" ebuilds" } else { &" ebuild" }];
            if totunknown > 0 {
                s.extend::<[&dyn Disp; 5]>([&", ", &st.cnt, &totunknown, &st.clr, &" unknown"]);
            }
            let tothidden = totcount.saturating_sub(first.min(last - 1));
            if tothidden > 0 {
                s.extend::<[&dyn Disp; 5]>([&", ", &st.cnt, &tothidden, &st.clr, &" hidden"]);
            }
            let e = FmtDur(totelapsed);
            if totelapsed > 0 {
                s.extend::<[&dyn Disp; 4]>([&", ", &e, &st.clr, &" elapsed"]);
            }
            tbl.row([&s,
                     &[&FmtDur(totpredict), &st.clr],
                     &[&"@ ", &st.dur, &FmtDate(now + totpredict)]]);
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
                        get_parse(args, "from", parse_date, st.date_offset)?,
                        get_parse(args, "to", parse_date, st.date_offset)?,
                        Show { merge: true, ..Show::default() },
                        args.get_many::<String>("search").unwrap_or_default().cloned().collect(),
                        args.get_flag("exact"))?;
    let last = *args.get_one("last").unwrap_or(&usize::MAX);
    let lim = *args.get_one("limit").unwrap();
    let avg = *args.get_one("avg").unwrap();
    let mut pkg_starts: HashMap<String, i64> = HashMap::new();
    let mut pkg_times: BTreeMap<String, Times> = BTreeMap::new();
    let mut pkg_errs: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut found = false;
    let mut tbl = Table::new(st).align_left(0).align_left(1).last(last);
    tbl.header(["Date", "Package", "Real", "Predicted", "Error"]);
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
                                tbl.row([&[&FmtDate(ts)],
                                         &[&st.merge, &p.ebuild_version()],
                                         &[&FmtDur(real)],
                                         &[],
                                         &[]])
                            }
                        },
                        pred => {
                            let err = (pred - real).abs() as f64 * 100.0 / real as f64;
                            if show.merge {
                                tbl.row([&[&FmtDate(ts)],
                                         &[&st.merge, &p.ebuild_version()],
                                         &[&FmtDur(real)],
                                         &[&FmtDur(pred)],
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
        let mut tbl = Table::new(st).align_left(0);
        tbl.header(["Package", "Error"]);
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
}
