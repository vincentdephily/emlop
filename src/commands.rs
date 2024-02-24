use crate::{datetime::*, parse::*, table::*, *};
use std::{collections::{BTreeMap, HashMap},
          io::stdin};

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_log(gc: &Conf, sc: &ConfLog) -> Result<bool, Error> {
    let hist = get_hist(&gc.logfile, gc.from, gc.to, sc.show, &sc.search, sc.exact)?;
    let mut merges: HashMap<String, i64> = HashMap::new();
    let mut unmerges: HashMap<String, i64> = HashMap::new();
    let mut found = 0;
    let mut sync_start: Option<i64> = None;
    let mut tbl = Table::new(gc).align_left(0).align_left(2).margin(2, " ").last(sc.last);
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
                tbl.row([&[&FmtDate(if sc.starttime { started } else { ts })],
                         &[&FmtDur(ts - started)],
                         &[&gc.merge, &p.ebuild_version()]]);
            },
            Hist::UnmergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if an unmerge started but never finished
                unmerges.insert(key, ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                found += 1;
                let started = unmerges.remove(key).unwrap_or(ts + 1);
                tbl.row([&[&FmtDate(if sc.starttime { started } else { ts })],
                         &[&FmtDur(ts - started)],
                         &[&gc.unmerge, &p.ebuild_version()]]);
            },
            Hist::SyncStart { ts } => {
                // Some sync starts have multiple entries in old logs
                sync_start = Some(ts);
            },
            Hist::SyncStop { ts, repo } => {
                if let Some(started) = sync_start.take() {
                    found += 1;
                    tbl.row([&[&FmtDate(if sc.starttime { started } else { ts })],
                             &[&FmtDur(ts - started)],
                             &[&gc.clr, &"Sync ", &repo]]);
                } else {
                    warn!("Sync stop without a start at {ts}");
                }
            },
        }
        if found >= sc.first {
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
    const fn new() -> Self {
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
pub fn cmd_stats(gc: &Conf, sc: &ConfStats) -> Result<bool, Error> {
    let hist = get_hist(&gc.logfile, gc.from, gc.to, sc.show, &sc.search, sc.exact)?;
    let mut tbls = Table::new(gc).align_left(0).align_left(1).margin(1, " ");
    tbls.header([sc.group.name(), "Repo", "Syncs", "Total time", "Predict time"]);
    let mut tblp = Table::new(gc).align_left(0).align_left(1).margin(1, " ");
    tblp.header([sc.group.name(),
                 "Package",
                 "Merges",
                 "Total time",
                 "Predict time",
                 "Unmerges",
                 "Total time",
                 "Predict time"]);
    let mut tblt = Table::new(gc).align_left(0).margin(1, " ");
    tblt.header([sc.group.name(),
                 "Merges",
                 "Total time",
                 "Average time",
                 "Unmerges",
                 "Total time",
                 "Average time"]);
    let mut merge_start: HashMap<String, i64> = HashMap::new();
    let mut unmerge_start: HashMap<String, i64> = HashMap::new();
    let mut pkg_time: BTreeMap<String, (Times, Times)> = BTreeMap::new();
    let mut sync_start: Option<i64> = None;
    let mut sync_time: BTreeMap<String, Times> = BTreeMap::new();
    let mut nextts = 0;
    let mut curts = 0;
    for p in hist {
        if !matches!(sc.group, Timespan::None) {
            let t = p.ts();
            if nextts == 0 {
                nextts = sc.group.next(t, gc.date_offset);
                curts = t;
            } else if t > nextts {
                let group = sc.group.at(curts, gc.date_offset);
                cmd_stats_group(gc, sc, &mut tbls, &mut tblp, &mut tblt, group, &sync_time,
                                &pkg_time);
                sync_time.clear();
                pkg_time.clear();
                nextts = sc.group.next(t, gc.date_offset);
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
                                             .or_insert((Times::new(), Times::new()));
                    times.insert(ts - start_ts);
                }
            },
            Hist::UnmergeStart { ts, key, .. } => {
                unmerge_start.insert(key, ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                if let Some(start_ts) = unmerge_start.remove(key) {
                    let (_, times) = pkg_time.entry(p.ebuild().to_owned())
                                             .or_insert((Times::new(), Times::new()));
                    times.insert(ts - start_ts);
                }
            },
            Hist::SyncStart { ts } => {
                // Some sync starts have multiple entries in old logs
                sync_start = Some(ts);
            },
            Hist::SyncStop { ts, repo } => {
                if let Some(start_ts) = sync_start.take() {
                    let times = sync_time.entry(repo).or_insert(Times::new());
                    times.insert(ts - start_ts);
                } else {
                    warn!("Sync stop without a start at {ts}")
                }
            },
        }
    }
    let group = sc.group.at(curts, gc.date_offset);
    cmd_stats_group(gc, sc, &mut tbls, &mut tblp, &mut tblt, group, &sync_time, &pkg_time);
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
fn cmd_stats_group(gc: &Conf,
                   sc: &ConfStats,
                   tbls: &mut Table<5>,
                   tblp: &mut Table<8>,
                   tblt: &mut Table<7>,
                   group: String,
                   sync_time: &BTreeMap<String, Times>,
                   pkg_time: &BTreeMap<String, (Times, Times)>) {
    // Syncs
    if sc.show.sync && !sync_time.is_empty() {
        for (repo, time) in sync_time {
            tbls.row([&[&group],
                      &[repo],
                      &[&gc.cnt, &time.count],
                      &[&FmtDur(time.tot)],
                      &[&FmtDur(time.pred(sc.lim, sc.avg))]]);
        }
    }
    // Packages
    if sc.show.pkg && !pkg_time.is_empty() {
        for (pkg, (merge, unmerge)) in pkg_time {
            tblp.row([&[&group],
                      &[&gc.pkg, pkg],
                      &[&gc.cnt, &merge.count],
                      &[&FmtDur(merge.tot)],
                      &[&FmtDur(merge.pred(sc.lim, sc.avg))],
                      &[&gc.cnt, &unmerge.count],
                      &[&FmtDur(unmerge.tot)],
                      &[&FmtDur(unmerge.pred(sc.lim, sc.avg))]]);
        }
    }
    // Totals
    if sc.show.tot && !pkg_time.is_empty() {
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
                  &[&gc.cnt, &merge_count],
                  &[&FmtDur(merge_time)],
                  &[&FmtDur(merge_time.checked_div(merge_count).unwrap_or(-1))],
                  &[&gc.cnt, &unmerge_count],
                  &[&FmtDur(unmerge_time)],
                  &[&FmtDur(unmerge_time.checked_div(unmerge_count).unwrap_or(-1))]]);
    }
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(gc: &Conf, sc: &ConfPred) -> Result<bool, Error> {
    let now = epoch_now();
    let last = if sc.show.tot { sc.last.saturating_add(1) } else { sc.last };
    let mut tbl = Table::new(gc).align_left(0).align_left(2).margin(2, " ").last(last);
    // TODO: should be able to extend inside sc
    let mut tmpdirs = sc.tmpdirs.clone();

    // Gather and print info about current merge process.
    let einfo = get_emerge(&mut tmpdirs);
    if einfo.cmds.is_empty()
       && std::io::stdin().is_terminal()
       && matches!(sc.resume, ResumeKind::No | ResumeKind::Auto)
    {
        tbl.row([&[&"No ongoing merge found"], &[], &[]]);
        return Ok(false);
    }
    if sc.show.emerge {
        for proc in &einfo.cmds {
            tbl.row([&[&proc], &[&FmtDur(now - proc.start)], &[]]);
        }
    }

    // Parse emerge log.
    let hist = get_hist(&gc.logfile, gc.from, gc.to, Show::m(), &vec![], false)?;
    let mut started: BTreeMap<Pkg, i64> = BTreeMap::new();
    let mut times: HashMap<String, Times> = HashMap::new();
    for p in hist {
        match p {
            Hist::MergeStart { ts, .. } => {
                started.insert(Pkg::new(p.ebuild(), p.version()), ts);
            },
            Hist::MergeStop { ts, .. } => {
                if let Some(start_ts) = started.remove(&Pkg::new(p.ebuild(), p.version())) {
                    let timevec = times.entry(p.ebuild().to_string()).or_insert(Times::new());
                    timevec.insert(ts - start_ts);
                }
            },
            _ => unreachable!("Should only receive Hist::{{Start,Stop}}"),
        }
    }

    // Build list of pending merges
    let pkgs: Vec<Pkg> = if std::io::stdin().is_terminal() {
        // From resume list
        let mut r = get_resume(sc.resume);
        // Plus specific emerge processes
        for p in einfo.pkgs.iter() {
            if !r.contains(p) {
                r.push(p.clone())
            }
        }
        // Plus emerge.log after main process start time, if we didn't see specific processes
        if einfo.pkgs.is_empty() {
            for (p, t) in started.iter() {
                if *t > einfo.start && !r.contains(p) {
                    r.push(p.clone())
                }
            }
        }
        r
    } else {
        // From portage's stdout
        get_pretend(stdin(), "STDIN")
    };
    trace!("pending: {pkgs:?}");

    // Gather and print per-package and indivudual stats.
    let mut totcount = 0;
    let mut totunknown = 0;
    let mut totpredict = 0;
    let mut totelapsed = 0;
    for p in pkgs {
        totcount += 1;
        // Find the elapsed time, if currently running
        let elapsed = match started.remove(&p) {
            Some(s) if einfo.pkgs.contains(&p) => now - s,
            Some(s) if einfo.pkgs.is_empty() && s > einfo.start => now - s,
            _ => 0,
        };

        // Find the predicted time and adjust counters
        let (fmtpred, pred) = match times.get(p.ebuild()) {
            Some(tv) => {
                let pred = tv.pred(sc.lim, sc.avg);
                (pred, pred)
            },
            None => {
                totunknown += 1;
                (-1, sc.unknown)
            },
        };
        totpredict += std::cmp::max(0, pred - elapsed);
        totelapsed += elapsed;

        // Done
        if sc.show.merge && totcount <= sc.first {
            if elapsed > 0 {
                let stage = get_buildlog(&p, &tmpdirs).unwrap_or_default();
                tbl.row([&[&gc.pkg, &p.ebuild_version()],
                         &[&FmtDur(fmtpred)],
                         &[&gc.clr, &"- ", &FmtDur(elapsed), &gc.clr, &stage]]);
            } else {
                tbl.row([&[&gc.pkg, &p.ebuild_version()], &[&FmtDur(fmtpred)], &[]]);
            }
        }
    }
    if totcount > 0 {
        if sc.show.tot {
            let mut s: Vec<&dyn Disp> = vec![&"Estimate for ",
                                             &gc.cnt,
                                             &totcount,
                                             &gc.clr,
                                             if totcount > 1 { &" ebuilds" } else { &" ebuild" }];
            if totunknown > 0 {
                s.extend::<[&dyn Disp; 5]>([&", ", &gc.cnt, &totunknown, &gc.clr, &" unknown"]);
            }
            let tothidden = totcount.saturating_sub(sc.first.min(last - 1));
            if tothidden > 0 {
                s.extend::<[&dyn Disp; 5]>([&", ", &gc.cnt, &tothidden, &gc.clr, &" hidden"]);
            }
            let e = FmtDur(totelapsed);
            if totelapsed > 0 {
                s.extend::<[&dyn Disp; 4]>([&", ", &e, &gc.clr, &" elapsed"]);
            }
            tbl.row([&s,
                     &[&FmtDur(totpredict), &gc.clr],
                     &[&"@ ", &gc.dur, &FmtDate(now + totpredict)]]);
        }
    } else {
        tbl.row([&[&"No pretended merge found"], &[], &[]]);
    }
    Ok(totcount > 0)
}

pub fn cmd_accuracy(gc: &Conf, sc: &ConfAccuracy) -> Result<bool, Error> {
    let hist = get_hist(&gc.logfile, gc.from, gc.to, Show::m(), &sc.search, sc.exact)?;
    let mut pkg_starts: HashMap<String, i64> = HashMap::new();
    let mut pkg_times: BTreeMap<String, Times> = BTreeMap::new();
    let mut pkg_errs: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut found = false;
    let mut tbl = Table::new(gc).align_left(0).align_left(1).last(sc.last);
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
                    let times = pkg_times.entry(p.ebuild().to_owned()).or_insert(Times::new());
                    let real = ts - start;
                    match times.pred(sc.lim, sc.avg) {
                        -1 => {
                            if sc.show.merge {
                                tbl.row([&[&FmtDate(ts)],
                                         &[&gc.merge, &p.ebuild_version()],
                                         &[&FmtDur(real)],
                                         &[],
                                         &[]])
                            }
                        },
                        pred => {
                            let err = (pred - real).abs() as f64 * 100.0 / real as f64;
                            if sc.show.merge {
                                tbl.row([&[&FmtDate(ts)],
                                         &[&gc.merge, &p.ebuild_version()],
                                         &[&FmtDur(real)],
                                         &[&FmtDur(pred)],
                                         &[&gc.cnt, &format!("{err:.1}%")]])
                            }
                            let errs = pkg_errs.entry(p.ebuild().to_owned()).or_default();
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
    if sc.show.tot {
        let mut tbl = Table::new(gc).align_left(0);
        tbl.header(["Package", "Error"]);
        for (p, e) in pkg_errs {
            let avg = e.iter().sum::<f64>() / e.len() as f64;
            tbl.row([&[&gc.pkg, &p], &[&gc.cnt, &format!("{avg:.1}%")]]);
        }
    }
    Ok(found)
}

pub fn cmd_complete(sc: &ConfComplete) -> Result<bool, Error> {
    let mut cli = build_cli_nocomplete();
    clap_complete::generate(sc.shell, &mut cli, "emlop", &mut std::io::stdout());
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
