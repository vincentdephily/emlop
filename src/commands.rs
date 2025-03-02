use crate::{datetime::*, parse::*, table::*, *};
use libc::pid_t;
use std::{collections::{BTreeMap, HashMap, HashSet},
          io::{stdin, IsTerminal}};

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_log(gc: Conf, sc: ConfLog) -> Result<bool, Error> {
    let hist = get_hist(&gc.logfile, gc.from, gc.to, sc.show, &sc.search, sc.exact)?;
    let mut merges: HashMap<String, (i64, bool)> = HashMap::new();
    let mut unmerges: HashMap<String, i64> = HashMap::new();
    let mut sync_start: Option<i64> = None;
    let mut found = 0;
    let h = ["Date", "Duration", "Package/Repo"];
    let mut tbl =
        Table::new(&gc).align_left(0).align_left(2).margin(2, " ").last(sc.last).header(h);
    for p in hist {
        match p {
            Hist::RunStart { ts, args, .. } => {
                found += 1;
                if found <= sc.first {
                    tbl.row([&[&FmtDate(ts)], &[], &[&"Emerge ", &args]]);
                }
            },
            Hist::MergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if a merge started but never finished
                merges.insert(key, (ts, false));
            },
            Hist::MergeBin { key, .. } => {
                if let Some((_, bin)) = merges.get_mut(&key) {
                    *bin = true;
                }
            },
            Hist::MergeStop { ts, ref key, .. } => {
                found += 1;
                let (started, bin) = merges.remove(key).unwrap_or((ts + 1, false));
                if found <= sc.first {
                    tbl.row([&[&FmtDate(if sc.starttime { started } else { ts })],
                             &[&FmtDur(ts - started)],
                             &[if bin { &gc.binmerge } else { &gc.merge }, &p.ebuild_version()]]);
                }
            },
            Hist::UnmergeStart { ts, key, .. } => {
                // This'll overwrite any previous entry, if an unmerge started but never finished
                unmerges.insert(key, ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                found += 1;
                let started = unmerges.remove(key).unwrap_or(ts + 1);
                if found <= sc.first {
                    tbl.row([&[&FmtDate(if sc.starttime { started } else { ts })],
                             &[&FmtDur(ts - started)],
                             &[&gc.unmerge, &p.ebuild_version()]]);
                }
            },
            Hist::SyncStart { ts } => {
                // Some sync starts have multiple entries in old logs
                sync_start = Some(ts);
            },
            Hist::SyncStop { ts, repo } => {
                found += 1;
                let started = sync_start.take().unwrap_or(ts + 1);
                if found <= sc.first {
                    tbl.row([&[&FmtDate(if sc.starttime { started } else { ts })],
                             &[&FmtDur(ts - started)],
                             &[&gc.sync, &"Sync ", &repo]]);
                }
            },
        }
        if !gc.showskip && found >= sc.first {
            break;
        }
    }
    if gc.showskip && found >= sc.first {
        tbl.skiprow(&[&gc.skip, &"(skip last ", &(found - sc.first), &")"]);
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

/// Classify emerge commands by looking at their args.
///
/// Note that some commands don't get logged at all, so this enum is quite limited.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum ArgKind {
    All,
    Merge,
    Clean,
    Sync,
}
impl ArgKind {
    fn new(args: &str) -> Self {
        for arg in args.split_ascii_whitespace() {
            match arg {
                "--deselect" | "--unmerge" | "--clean" | "--depclean" => return Self::Clean,
                "--sync" => return Self::Sync,
                _ => (),
            }
        }
        Self::Merge
    }
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
pub fn cmd_stats(gc: Conf, sc: ConfStats) -> Result<bool, Error> {
    let hist = get_hist(&gc.logfile, gc.from, gc.to, sc.show, &sc.search, sc.exact)?;
    let moves = PkgMoves::new(&Mtimedb::new());
    let h = [sc.group.name(), "Logged emerges", "Install/Update", "Unmerge/Clean", "Sync"];
    let mut tblc = Table::new(&gc).align_left(0).margin(1, " ").header(h);
    let h = [sc.group.name(), "Repo", "Syncs", "Total time", "Predict time"];
    let mut tbls = Table::new(&gc).align_left(0).align_left(1).margin(1, " ").header(h);
    let h = [sc.group.name(),
             "Package",
             "Merges",
             "Total time",
             "Predict time",
             "Binmerges",
             "Total time",
             "Predict time",
             "Unmerges",
             "Total time",
             "Predict time"];
    let mut tblp = Table::new(&gc).align_left(0).align_left(1).margin(1, " ").header(h);
    let h = [sc.group.name(),
             "Merges",
             "Total time",
             "Average time",
             "Binmerges",
             "Total time",
             "Average time",
             "Unmerges",
             "Total time",
             "Average time"];
    let mut tblt = Table::new(&gc).align_left(0).margin(1, " ").header(h);
    let mut merge_start: HashMap<String, (i64, bool)> = HashMap::new();
    let mut unmerge_start: HashMap<String, i64> = HashMap::new();
    let mut pkg_time: BTreeMap<String, (Times, Times, Times)> = BTreeMap::new();
    let mut sync_start: Option<i64> = None;
    let mut sync_time: BTreeMap<String, Times> = BTreeMap::new();
    let mut run_args: BTreeMap<ArgKind, usize> = BTreeMap::new();
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
                cmd_stats_group(&gc, &sc, &mut tblc, &mut tbls, &mut tblp, &mut tblt, group,
                                &run_args, &sync_time, &pkg_time);
                sync_time.clear();
                pkg_time.clear();
                run_args.clear();
                nextts = sc.group.next(t, gc.date_offset);
                curts = t;
            }
        }
        match p {
            Hist::RunStart { args, .. } => {
                *run_args.entry(ArgKind::All).or_insert(0) += 1;
                *run_args.entry(ArgKind::new(&args)).or_insert(0) += 1;
            },
            Hist::MergeStart { ts, key, .. } => {
                merge_start.insert(moves.get(key), (ts, false));
            },
            Hist::MergeBin { key, .. } => {
                if let Some((_, bin)) = merge_start.get_mut(&key) {
                    *bin = true;
                }
            },
            Hist::MergeStop { ts, ref key, .. } => {
                if let Some((start_ts, bin)) = merge_start.remove(moves.get_ref(key)) {
                    let (tc, tb, _) =
                        pkg_time.entry(moves.get(p.take_ebuild()))
                                .or_insert((Times::new(), Times::new(), Times::new()));
                    if bin {
                        tb.insert(ts - start_ts);
                    } else {
                        tc.insert(ts - start_ts);
                    }
                }
            },
            Hist::UnmergeStart { ts, key, .. } => {
                unmerge_start.insert(moves.get(key), ts);
            },
            Hist::UnmergeStop { ts, ref key, .. } => {
                if let Some(start_ts) = unmerge_start.remove(moves.get_ref(key)) {
                    let (_, _, times) =
                        pkg_time.entry(moves.get(p.take_ebuild()))
                                .or_insert((Times::new(), Times::new(), Times::new()));
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
    cmd_stats_group(&gc, &sc, &mut tblc, &mut tbls, &mut tblp, &mut tblt, group, &run_args,
                    &sync_time, &pkg_time);
    // Controlled drop to ensure table order and insert blank lines
    let (ec, es, ep, et) = (!tblc.is_empty(), !tbls.is_empty(), !tblp.is_empty(), !tblt.is_empty());
    drop(tblc);
    if ec && es {
        println!();
    }
    drop(tbls);
    if (ec || es) && ep {
        println!();
    }
    drop(tblp);
    if (ec || es || ep) && et {
        println!();
    }
    drop(tblt);
    Ok(!pkg_time.is_empty() || !sync_time.is_empty())
}

// Reducing the arg count here doesn't seem worth it, for either readability or performance
#[allow(clippy::too_many_arguments)]
fn cmd_stats_group(gc: &Conf,
                   sc: &ConfStats,
                   tblc: &mut Table<5>,
                   tbls: &mut Table<5>,
                   tblp: &mut Table<11>,
                   tblt: &mut Table<10>,
                   group: String,
                   run_args: &BTreeMap<ArgKind, usize>,
                   sync_time: &BTreeMap<String, Times>,
                   pkg_time: &BTreeMap<String, (Times, Times, Times)>) {
    // Commands
    if sc.show.run && !run_args.is_empty() {
        tblc.row([&[&group],
                  &[&gc.cnt, run_args.get(&ArgKind::All).unwrap_or(&0)],
                  &[&gc.cnt, run_args.get(&ArgKind::Merge).unwrap_or(&0)],
                  &[&gc.cnt, run_args.get(&ArgKind::Clean).unwrap_or(&0)],
                  &[&gc.cnt, run_args.get(&ArgKind::Sync).unwrap_or(&0)]]);
    }
    // Syncs
    if sc.show.sync && !sync_time.is_empty() {
        for (repo, time) in sync_time {
            tbls.row([&[&group],
                      &[&gc.sync, repo],
                      &[&gc.cnt, &time.count],
                      &[&FmtDur(time.tot)],
                      &[&FmtDur(time.pred(sc.lim, sc.avg))]]);
        }
    }
    // Packages
    if sc.show.pkg && !pkg_time.is_empty() {
        for (pkg, (merge, binmerge, unmerge)) in pkg_time {
            tblp.row([&[&group],
                      &[&gc.pkg, pkg],
                      &[&gc.cnt, &merge.count],
                      &[&FmtDur(merge.tot)],
                      &[&FmtDur(merge.pred(sc.lim, sc.avg))],
                      &[&gc.cnt, &binmerge.count],
                      &[&FmtDur(binmerge.tot)],
                      &[&FmtDur(binmerge.pred(sc.lim, sc.avg))],
                      &[&gc.cnt, &unmerge.count],
                      &[&FmtDur(unmerge.tot)],
                      &[&FmtDur(unmerge.pred(sc.lim, sc.avg))]]);
        }
    }
    // Totals
    if sc.show.tot && !pkg_time.is_empty() {
        let mut merge_time = 0;
        let mut merge_count = 0;
        let mut binmerge_time = 0;
        let mut binmerge_count = 0;
        let mut unmerge_time = 0;
        let mut unmerge_count = 0;
        for (merge, binmerge, unmerge) in pkg_time.values() {
            merge_time += merge.tot;
            merge_count += merge.count;
            binmerge_time += binmerge.tot;
            binmerge_count += binmerge.count;
            unmerge_time += unmerge.tot;
            unmerge_count += unmerge.count;
        }
        tblt.row([&[&group],
                  &[&gc.cnt, &merge_count],
                  &[&FmtDur(merge_time)],
                  &[&FmtDur(merge_time.checked_div(merge_count).unwrap_or(-1))],
                  &[&gc.cnt, &binmerge_count],
                  &[&FmtDur(binmerge_time)],
                  &[&FmtDur(binmerge_time.checked_div(binmerge_count).unwrap_or(-1))],
                  &[&gc.cnt, &unmerge_count],
                  &[&FmtDur(unmerge_time)],
                  &[&FmtDur(unmerge_time.checked_div(unmerge_count).unwrap_or(-1))]]);
    }
}

/// Count processes in tree, including given proces
fn proc_count(procs: &ProcList, pid: pid_t) -> usize {
    let mut count = 1;
    for child in procs.iter().filter(|(_, p)| p.ppid == pid).map(|(pid, _)| pid) {
        count += proc_count(procs, *child);
    }
    count
}

/// Display proces tree
fn proc_rows(now: i64,
             tbl: &mut Table<3>,
             procs: &ProcList,
             pid: pid_t,
             depth: usize,
             gc: &Conf,
             sc: &ConfPred) {
    // This should always succeed because we're getting pid from procs, but to allow experiments we
    // warn instead of panic/ignore.
    let proc = match procs.get(&pid) {
        Some(p) => p,
        None => {
            error!("Could not find proces {pid}");
            return;
        },
    };
    // Print current level
    if depth < sc.pdepth {
        tbl.row([&[&FmtProc(proc, depth, sc.pwidth)], &[&FmtDur(now - proc.start)], &[]]);
    }
    // Either recurse with children...
    if depth + 1 < sc.pdepth {
        for child in procs.iter().filter(|(_, p)| p.ppid == pid).map(|(pid, _)| pid) {
            proc_rows(now, tbl, procs, *child, depth + 1, gc, sc);
        }
    }
    // ...or print skipped rows
    else if gc.showskip {
        let count = proc_count(procs, pid) - 1;
        if count > 0 {
            tbl.skiprow(&[&"  ".repeat(depth + 1), &gc.skip, &"(skip ", &count, &" below)"]);
        }
    }
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(gc: Conf, mut sc: ConfPred) -> Result<bool, Error> {
    let now = epoch_now();
    let last = if sc.show.tot { sc.last.saturating_add(1) } else { sc.last };
    let mut tbl = Table::new(&gc).align_left(0).align_left(2).margin(2, " ").last(last);

    // Gather and print info about current merge process.
    let procs = get_all_proc(&mut sc.tmpdirs);
    let einfo = get_emerge(&procs);
    if einfo.roots.is_empty()
       && std::io::stdin().is_terminal()
       && matches!(sc.resume, ResumeKind::No | ResumeKind::Auto)
    {
        tbl.row([&[&"No ongoing merge found"], &[], &[]]);
        return Ok(false);
    }
    if sc.show.run {
        for p in einfo.roots {
            proc_rows(now, &mut tbl, &procs, p, 0, &gc, &sc);
        }
    }

    // Parse emerge log.
    let hist = get_hist(&gc.logfile, gc.from, gc.to, Show::m(), &vec![], false)?;
    let mdb = Mtimedb::new();
    let moves = PkgMoves::new(&mdb);
    let mut started: BTreeMap<String, (i64, bool)> = BTreeMap::new();
    let mut times: HashMap<(String, bool), Times> = HashMap::new();
    for p in hist {
        match p {
            Hist::MergeStart { ts, key, .. } => {
                started.insert(moves.get(key), (ts, false));
            },
            Hist::MergeBin { key, .. } => {
                if let Some((_, bin)) = started.get_mut(moves.get_ref(&key)) {
                    *bin = true;
                }
            },
            Hist::MergeStop { ts, ref key, .. } => {
                if let Some((start_ts, bin)) = started.remove(moves.get_ref(key)) {
                    let timevec =
                        times.entry((moves.get(p.take_ebuild()), bin)).or_insert(Times::new());
                    timevec.insert(ts - start_ts);
                }
            },
            _ => unreachable!("Should only receive Hist::{{Start,Step,Stop}}"),
        }
    }

    // Build list of pending merges
    let pkgs: Vec<Pkg> = if std::io::stdin().is_terminal() {
        // From resume list
        let mut r = get_resume(sc.resume, &mdb);
        // Plus specific emerge processes
        for p in einfo.pkgs.iter() {
            if !r.contains(p) {
                r.push(p.clone())
            }
        }
        // Plus emerge.log after main process start time, if we didn't see specific processes
        if einfo.pkgs.is_empty() {
            for (p, (t, b)) in started.iter() {
                if *t > einfo.start && r.iter().all(|r| r.ebuild_version() != p) {
                    r.push(Pkg::try_new(p, *b).expect("started key should parse as Pkg"))
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
    let mut totbuild = 0;
    let mut totbin = 0;
    let mut totunknown = 0;
    let mut totpredict = 0;
    let mut totelapsed = 0;
    for p in pkgs {
        totcount += 1;
        if p.bin {
            totbin += 1;
        } else {
            totbuild += 1;
        }
        // Find the elapsed time, if currently running
        let elapsed = match started.remove(p.ebuild_version()) {
            Some((s, _)) if einfo.pkgs.contains(&p) => now - s,
            Some((s, _)) if einfo.pkgs.is_empty() && s > einfo.start => now - s,
            _ => 0,
        };

        // Find the predicted time and adjust counters
        let (fmtpred, pred) = match times.get(&(p.ebuild().to_string(), p.bin)) {
            Some(tv) => {
                let pred = tv.pred(sc.lim, sc.avg);
                (pred, pred)
            },
            None => {
                totunknown += 1;
                let u = if p.bin { sc.unknownb } else { sc.unknownc };
                (i64::MIN + u, u)
            },
        };
        totpredict += std::cmp::max(0, pred - elapsed);
        totelapsed += elapsed;

        // Done
        if sc.show.merge && totcount <= sc.first {
            if elapsed > 0 {
                let stage = get_buildlog(&p, &sc.tmpdirs).unwrap_or_default();
                tbl.row([&[if p.bin { &gc.binpkg } else { &gc.pkg }, &p.ebuild_version()],
                         &[&FmtDur(fmtpred)],
                         &[&gc.clr, &"- ", &FmtDur(elapsed), &gc.clr, &stage]]);
            } else {
                tbl.row([&[if p.bin { &gc.binpkg } else { &gc.pkg }, &p.ebuild_version()],
                         &[&FmtDur(fmtpred)],
                         &[]]);
            }
        }
    }
    let lastskip = totcount.saturating_sub(sc.first);
    if sc.show.merge && gc.showskip && lastskip > 0 {
        tbl.skiprow(&[&gc.skip, &"(skip last ", &lastskip, &")"]);
    }
    // Print summary line
    if totcount == 0 {
        tbl.row([&[&"No pretended merge found"], &[], &[]]);
    } else if sc.show.tot {
        let mut s: Vec<&dyn Disp> = vec![&"Estimate for "];
        if totbuild > 0 {
            s.extend([&gc.cnt as &dyn Disp,
                      &totbuild,
                      &gc.clr,
                      if totbuild > 1 { &" builds" } else { &" build" }]);
        }
        if totbin > 0 {
            s.extend([if totbuild > 0 { &", " } else { &"" } as &dyn Disp,
                      &gc.cnt,
                      &totbin,
                      &gc.clr,
                      if totbin > 1 { &" binaries" } else { &" binary" }]);
        }
        if totunknown > 0 {
            s.extend([&", " as &dyn Disp, &gc.cnt, &totunknown, &gc.clr, &" unknown"]);
        }
        let e = FmtDur(totelapsed);
        if totelapsed > 0 {
            s.extend([&", " as &dyn Disp, &e, &gc.clr, &" elapsed"]);
        }
        tbl.row([&s,
                 &[&FmtDur(totpredict), &gc.clr],
                 &[&"@ ", &gc.dur, &FmtDate(now + totpredict)]]);
    }
    Ok(totcount > 0)
}

pub fn cmd_accuracy(gc: Conf, sc: ConfAccuracy) -> Result<bool, Error> {
    let hist = get_hist(&gc.logfile, gc.from, gc.to, Show::m(), &sc.search, sc.exact)?;
    let mut pkg_starts: HashMap<String, i64> = HashMap::new();
    let mut pkg_times: BTreeMap<String, Times> = BTreeMap::new();
    let mut pkg_errs: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut found = false;
    let h = ["Date", "Package", "Real", "Predicted", "Error"];
    let mut tbl = Table::new(&gc).align_left(0).align_left(1).last(sc.last).header(h);
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
        let mut tbl = Table::new(&gc).align_left(0).header(["Package", "Error"]);
        for (p, e) in pkg_errs {
            let avg = e.iter().sum::<f64>() / e.len() as f64;
            tbl.row([&[&gc.pkg, &p], &[&gc.cnt, &format!("{avg:.1}%")]]);
        }
    }
    Ok(found)
}

pub fn cmd_complete(gc: Conf, sc: ConfComplete) -> Result<bool, Error> {
    // Generate standard clap completions
    #[cfg(feature = "clap_complete")]
    if let Some(s) = &sc.shell {
        let mut cli = build_cli();
        let shell = clap_complete::Shell::from_str(s).expect("Unsupported shell");
        clap_complete::generate(shell, &mut cli, "emlop", &mut std::io::stdout());
        return Ok(true);
    }
    // Look for (un)merged matching packages in the log and print each once
    let term: Vec<_> = sc.pkg.map_or(vec![], |p| vec![p]);
    let hist = get_hist(&gc.logfile, gc.from, gc.to, Show::m(), &term, false)?;
    let mut pkgs: HashSet<String> = HashSet::new();
    for p in hist {
        if let Hist::MergeStart { .. } = p {
            let e = p.take_ebuild();
            if !pkgs.contains(&e) {
                println!("{}", e);
                pkgs.insert(e);
            }
        }
    }
    Ok(true)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::procs;

    #[test]
    fn averages() {
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

    /// Shows the whole system's processes.
    /// Mainly useful as an interactive test, use `cargo test -- --nocapture procs_pid1`.
    #[test]
    fn procs_pid1() {
        let (gc, mut sc) = ConfPred::from_str("emlop p --pdepth 4");
        let mut tbl = Table::new(&gc).align_left(0).align_left(2).margin(2, " ");
        let now = epoch_now();
        let procs = get_all_proc(&mut sc.tmpdirs);
        proc_rows(now, &mut tbl, &procs, 1, 0, &gc, &sc);
        println!("{}", tbl.to_string());
    }

    /// Check indentation and skipping
    #[test]
    fn procs_hierarchy() {
        let (gc, sc) = ConfPred::from_str("emlop p --pdepth 3 --color=n --output=c --showskip");
        let mut tbl = Table::new(&gc).align_left(0).align_left(2).margin(2, " ");
        let procs = procs(&[(ProcKind::Other, "a", 1, 0),
                            (ProcKind::Other, "a.a", 2, 1),
                            (ProcKind::Other, "a.b", 3, 1),
                            (ProcKind::Other, "a.a.a", 4, 2),
                            (ProcKind::Other, "a.a.b", 5, 2),
                            (ProcKind::Other, "a.b.a", 6, 3),
                            // basic skip
                            (ProcKind::Other, "a.a.a.a", 7, 4),
                            // nested/sibling skip
                            (ProcKind::Other, "a.a.b.a", 8, 5),
                            (ProcKind::Other, "a.a.b.a.a", 9, 8),
                            (ProcKind::Other, "a.a.b.b", 10, 5)]);
        let out = r#"1 a                   9
  2 a.a               8
    4 a.a.a           6
      (skip 1 below)   
    5 a.a.b           5
      (skip 3 below)   
  3 a.b               7
    6 a.b.a           4
"#;
        proc_rows(10, &mut tbl, &procs, 1, 0, &gc, &sc);
        assert_eq!(tbl.to_string(), out);
    }
}
