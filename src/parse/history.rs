//! Handles emerge.log parsing.
//!
//! Use `new_hist()` to start parsing and retrieve `Hist` enums.

use crate::{datetime::fmt_utctime, Show, TimeBound};
use anyhow::{bail, ensure, Context, Error};
use flate2::read::GzDecoder;
use log::*;
use memchr::{memchr, memrchr, memrchr2};
use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use std::{fs::File,
          io::{BufRead, BufReader},
          str::from_utf8,
          sync::mpsc::{sync_channel, Receiver, SyncSender},
          thread,
          time::Instant};

/// Items sent on the channel returned by `new_hist()`.
#[derive(Debug)]
pub enum Hist {
    /// Emerge run started (might never complete).
    // There's no RunStop, because matching a Stop to the correct Start is too unreliable
    RunStart { ts: i64, args: String },
    /// Merge started (might never complete).
    MergeStart { ts: i64, key: String, pos: usize },
    /// Merge is a binary merge.
    MergeBin { ts: i64, key: String, pos: usize },
    /// Merge completed.
    MergeStop { ts: i64, key: String, pos: usize },
    /// Unmerge started (might never complete).
    UnmergeStart { ts: i64, key: String, pos: usize },
    /// Unmerge completed.
    UnmergeStop { ts: i64, key: String, pos: usize },
    /// Sync started (might never complete).
    SyncStart { ts: i64 },
    /// Sync completed.
    SyncStop { ts: i64, repo: String },
}
impl Hist {
    pub fn ebuild(&self) -> &str {
        match self {
            Self::MergeStart { key, pos, .. }
            | Self::MergeBin { key, pos, .. }
            | Self::MergeStop { key, pos, .. }
            | Self::UnmergeStart { key, pos, .. }
            | Self::UnmergeStop { key, pos, .. } => &key[..(*pos - 1)],
            _ => unreachable!("No ebuild for {:?}", self),
        }
    }
    pub fn take_ebuild(self) -> String {
        match self {
            Self::MergeStart { mut key, pos, .. }
            | Self::MergeBin { mut key, pos, .. }
            | Self::MergeStop { mut key, pos, .. }
            | Self::UnmergeStart { mut key, pos, .. }
            | Self::UnmergeStop { mut key, pos, .. } => {
                key.truncate(pos - 1);
                key
            },
            _ => unreachable!("No ebuild for {:?}", self),
        }
    }
    #[cfg(test)]
    pub fn version(&self) -> &str {
        match self {
            Self::MergeStart { key, pos, .. }
            | Self::MergeBin { key, pos, .. }
            | Self::MergeStop { key, pos, .. }
            | Self::UnmergeStart { key, pos, .. }
            | Self::UnmergeStop { key, pos, .. } => &key[*pos..],
            _ => unreachable!("No version for {:?}", self),
        }
    }
    pub fn ebuild_version(&self) -> &str {
        match self {
            Self::MergeStart { key, .. }
            | Self::MergeBin { key, .. }
            | Self::MergeStop { key, .. }
            | Self::UnmergeStart { key, .. }
            | Self::UnmergeStop { key, .. } => key,
            _ => unreachable!("No ebuild/version for {:?}", self),
        }
    }
    pub const fn ts(&self) -> i64 {
        match self {
            Self::RunStart { ts, .. }
            | Self::MergeStart { ts, .. }
            | Self::MergeBin { ts, .. }
            | Self::MergeStop { ts, .. }
            | Self::UnmergeStart { ts, .. }
            | Self::UnmergeStop { ts, .. }
            | Self::SyncStart { ts, .. }
            | Self::SyncStop { ts, .. } => *ts,
        }
    }
}

/// Open maybe-compressed file, returning a BufReader
fn open_any_buffered(name: &str) -> Result<BufReader<Box<dyn std::io::Read + Send>>, Error> {
    let reader = File::open(name).with_context(|| format!("Cannot open {name:?}"))?;
    if name.ends_with(".gz") {
        let gz = GzDecoder::new(reader);
        ensure!(gz.header().is_some(), "Cannot open {name:?}: invalid gzip header");
        Ok(BufReader::new(Box::new(gz)))
    } else {
        Ok(BufReader::new(Box::new(reader)))
    }
}

/// Parse emerge log into a channel of `Parsed` enums.
pub fn get_hist(file: &str,
                min: TimeBound,
                max: TimeBound,
                show: Show,
                search_terms: &Vec<String>,
                search_exact: bool)
                -> Result<Receiver<Hist>, Error> {
    trace!("Show: {show}");
    let now = Instant::now();
    let logfile = file.to_owned();
    let (ts_min, ts_max) = filter_ts(file, min, max)?;
    let filter = FilterStr::try_new(search_terms, search_exact)?;
    let mut buf = open_any_buffered(file)?;
    let (tx, rx): (SyncSender<Hist>, Receiver<Hist>) = sync_channel(256);
    thread::spawn(move || {
        let show_merge = show.merge || show.pkg || show.tot;
        let show_unmerge = show.unmerge || show.pkg || show.tot;
        let mut prev_t = 0;
        let mut curline = 1;
        let mut line = Vec::with_capacity(255);
        loop {
            match buf.read_until(b'\n', &mut line) {
                // End of file
                Ok(0) => break,
                // Got a line, see if one of the funs match it
                Ok(_) => {
                    if let Some((t, s)) = parse_ts(&line, ts_min, ts_max) {
                        if prev_t > t {
                            warn!("{logfile}:{curline}: System clock jump: {} -> {}",
                                  fmt_utctime(prev_t),
                                  fmt_utctime(t));
                        }
                        prev_t = t;
                        if let Some(found) = parse_mergestart(show_merge, t, s, &filter) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_mergestop(show_merge, t, s, &filter) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_unmergestart(show_unmerge, t, s, &filter)
                        {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_unmergestop(show_unmerge, t, s, &filter) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_runstart(show.run, t, s) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_syncstart(show.sync, t, s) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_syncstop(show.sync, t, s, &filter) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_mergebin(show_merge, t, s, &filter) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        }
                    }
                },
                // System read error...
                Err(e) => warn!("{logfile}:{curline}: {e}"),
            }
            line.clear();
            curline += 1;
        }
        debug!("Parsed {curline} {logfile} lines in {:?}", now.elapsed());
    });
    Ok(rx)
}

/// Return min/max timestamp depending on options.
fn filter_ts(file: &str, min: TimeBound, max: TimeBound) -> Result<(i64, i64), Error> {
    // Parse emerge log into a Vec of emerge command starts
    // This is a specialized version of get_hist(), about 20% faster for this usecase
    let mut runs = vec![];
    if matches!(min, TimeBound::Run(_)) || matches!(max, TimeBound::Run(_)) {
        let mut buf = open_any_buffered(file)?;
        let mut line = Vec::with_capacity(255);
        loop {
            match buf.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some((t, s)) = parse_ts(&line, i64::MIN, i64::MAX) {
                        if s.starts_with(b"*** emerge") {
                            runs.push(t)
                        }
                    }
                },
                Err(_) => (),
            }
            line.clear();
        }
    }
    // Convert to Option<int>
    let min = match min {
        TimeBound::Run(n) => runs.iter().rev().nth(n).copied(),
        TimeBound::Unix(n) => Some(n),
        TimeBound::None => None,
    };
    let max = match max {
        TimeBound::Run(n) => runs.get(n).copied(),
        TimeBound::Unix(n) => Some(n),
        TimeBound::None => None,
    };
    // Check and log bounds, return result
    match (min, max) {
        (None, None) => trace!("Date: None"),
        (Some(a), None) => trace!("Date: after {}", fmt_utctime(a)),
        (None, Some(b)) => trace!("Date: before {}", fmt_utctime(b)),
        (Some(a), Some(b)) if a < b => {
            trace!("Date: between {} and {}", fmt_utctime(a), fmt_utctime(b))
        },
        (Some(a), Some(b)) => {
            bail!("Invalid date filter: {} <= {}, did you swap --to and --from ?",
                  fmt_utctime(a),
                  fmt_utctime(b))
        },
    }
    Ok((min.unwrap_or(i64::MIN), max.unwrap_or(i64::MAX)))
}

/// Matches package/repo depending on options.
enum FilterStr {
    True,
    Eq { a: Vec<String>, b: Vec<String>, c: Vec<String> },
    Re1 { r: Regex },
    Re { r: RegexSet },
}
impl FilterStr {
    fn try_new(terms: &Vec<String>, exact: bool) -> Result<Self, regex::Error> {
        trace!("Search: {terms:?} {exact}");
        Ok(match (terms.len(), exact) {
            (0, _) => Self::True,
            (_, true) => {
                let (b, c) = terms.iter().cloned().partition(|s| s.contains('/'));
                Self::Eq { a: terms.clone(),
                           b,
                           c: c.into_iter().map(|s| format!("/{s}")).collect() }
            },
            (1, false) => {
                Self::Re1 { r: RegexBuilder::new(&terms[0]).case_insensitive(true).build()? }
            },
            (_, false) => {
                Self::Re { r: RegexSetBuilder::new(terms).case_insensitive(true).build()? }
            },
        })
    }
    fn match_pkg(&self, s: &str) -> bool {
        match &self {
            Self::True => true,
            Self::Eq { b, c, .. } => b.iter().any(|e| e == s) || c.iter().any(|e| s.ends_with(e)),
            Self::Re1 { r } => r.is_match(s),
            Self::Re { r } => r.is_match(s),
        }
    }
    fn match_str(&self, s: &str) -> bool {
        match &self {
            Self::True => true,
            Self::Eq { a, .. } => a.iter().any(|e| e == s),
            Self::Re1 { r } => r.is_match(s),
            Self::Re { r } => r.is_match(s),
        }
    }
}


/// Find position of "version" in "categ/name-version" and filter on pkg name
fn parse_version(atom: &str, filter: &FilterStr) -> Option<usize> {
    let batom = atom.as_bytes();
    let mut pos = 0;
    loop {
        pos += memchr(b'-', &batom[pos..])? + 1;
        if pos > 1 && batom.get(pos)?.is_ascii_digit() {
            return filter.match_pkg(&atom[..(pos - 1)]).then_some(pos);
        }
    }
}

/// Parse and filter timestamp, trim line
fn parse_ts(line: &[u8], min: i64, max: i64) -> Option<(i64, &[u8])> {
    use atoi::FromRadix10;
    match i64::from_radix_10(line) {
        (ts, n) if n != 0 && ts >= min && ts <= max => {
            let mut line = &line[(n + 1)..];
            while let [b' ', rest @ ..] = line {
                line = rest;
            }
            if let [rest @ .., b'\n'] = line {
                line = rest;
            }
            Some((ts, line))
        },
        _ => None,
    }
}

/// *** emerge --update --ask --deep --reinstall=changed-use --regex-search-auto=y --verbose system
fn parse_runstart(enabled: bool, ts: i64, line: &[u8]) -> Option<Hist> {
    if !enabled || !line.starts_with(b"*** emer") {
        return None;
    }
    Some(Hist::RunStart { ts, args: from_utf8(&line[11..]).ok()?.to_owned() })
}

/// >>> emerge (1 of 1) www-client/falkon-24.08.3 to /
fn parse_mergestart(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b">>> emer") {
        return None;
    }
    let mut tokens = line.split(|c| *c == b' ');
    let atom = from_utf8(tokens.nth(5)?).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(Hist::MergeStart { ts, key: atom.to_owned(), pos })
}

/// === (1 of 1) Merging Binary (www-client/falkon-24.08.3::/var/cache/binpkgs/www-client/falkon-24.08.3.gpkg.tar)
fn parse_mergebin(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b"=== (") {
        return None;
    }
    let p1 = memchr(b')', line)?;
    if !&line[(p1 + 2)..].starts_with(b"Merging Bin") {
        return None;
    }
    let p2 = memchr(b'(', &line[p1..])? + p1 + 1;
    let p3 = memchr(b':', &line[p2..])? + p2;
    let atom = from_utf8(&line[p2..p3]).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(Hist::MergeBin { ts, key: atom.to_owned(), pos })
}

/// ::: completed emerge (1 of 1) www-client/falkon-24.08.3 to /
fn parse_mergestop(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b"::: comp") {
        return None;
    }
    let mut tokens = line.split(|c| *c == b' ');
    let atom = from_utf8(tokens.nth(6)?).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(Hist::MergeStop { ts, key: atom.to_owned(), pos })
}

/// === Unmerging... (app-portage/getuto-1.13)
fn parse_unmergestart(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b"=== Unmer") {
        return None;
    }
    let p1 = memchr(b'(', line)? + 1;
    let atom = from_utf8(&line[p1..line.len() - 1]).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(Hist::UnmergeStart { ts, key: atom.to_owned(), pos })
}

/// >>> unmerge success: www-client/falkon-24.08.3
fn parse_unmergestop(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b">>> unmerge succ") {
        return None;
    }
    let p1 = memrchr(b' ', line)? + 1;
    let atom = from_utf8(&line[p1..]).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(Hist::UnmergeStop { ts, key: atom.to_owned(), pos })
}

/// >>> Syncing repository 'gentoo' into '/usr/portage'...
/// >>> Starting rsync with rsync://91.186.30.235/gentoo-portage
fn parse_syncstart(enabled: bool, ts: i64, line: &[u8]) -> Option<Hist> {
    // Old portage logs 'Starting rsync with <url>', new portage logs 'Syncing repository <name>',
    // and intermediate versions log both. This makes it hard to properly match a start repo string
    // to a stop repo string across portage versions. Since syncs are not concurrent, we simply
    // ignore the start repo string.
    if enabled
       && (line.starts_with(b">>> Syncing")
           || line.starts_with(b">>> Starting rsync")
           || line.starts_with(b">>> starting rsync"))
    {
        Some(Hist::SyncStart { ts })
    } else {
        None
    }
}

/// === Sync completed with rsync://209.177.148.226/gentoo-portage
/// === Sync completed for gentoo
fn parse_syncstop(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    // Old portage logs 'completed with <url>', new portage logs 'completed for <name>'
    if !enabled || !line.starts_with(b"=== Sync comp") {
        return None;
    }
    let pos = memrchr2(b' ', b'/', line)? + 1;
    let repo = from_utf8(&line[pos..]).ok()?;
    filter.match_str(repo).then_some(Hist::SyncStop { ts, repo: repo.to_owned() })
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArgParse;
    use std::collections::HashMap;

    /// This checks parsing the given emerge.log.
    fn chk_hist(file: &str,
                show: &str,
                filter_mints: Option<i64>,
                filter_maxts: Option<i64>,
                filter_terms: Vec<String>,
                exact: bool,
                expect_counts: Vec<(&str, usize)>) {
        // Setup
        let (mints, maxts) = match file {
            "10000" => (1517609348, 1520991402),
            "all" => (1483228800, 1483747200),
            "badtimestamp" => (1327867709, 1327871057),
            "badversion" => (1327867709, 1327871057),
            "nullbytes" => (1327867709, 1327871057),
            "shortline" => (1327867709, 1327871057),
            o => unimplemented!("Unknown test log file {:?}", o),
        };
        let hist = get_hist(&format!("tests/emerge.{}.log", file),
                            filter_mints.map_or(TimeBound::None, |n| TimeBound::Unix(n)),
                            filter_maxts.map_or(TimeBound::None, |n| TimeBound::Unix(n)),
                            Show::parse(&String::from(show), "rptsmua", "test").unwrap(),
                            &filter_terms,
                            exact).unwrap();
        let re_atom = Regex::new("^[a-zA-Z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap();
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap();
        let mut counts: HashMap<String, usize> = HashMap::new();
        // Check that all items look valid
        for p in hist {
            let (kind, ts, ebuild, version) = match p {
                Hist::RunStart { ts, .. } => ("RStart", ts, "c/e", "1"),
                Hist::MergeStart { ts, .. } => ("MStart", ts, p.ebuild(), p.version()),
                Hist::MergeBin { ts, .. } => ("MBin", ts, p.ebuild(), p.version()),
                Hist::MergeStop { ts, .. } => ("MStop", ts, p.ebuild(), p.version()),
                Hist::UnmergeStart { ts, .. } => ("UStart", ts, p.ebuild(), p.version()),
                Hist::UnmergeStop { ts, .. } => ("UStop", ts, p.ebuild(), p.version()),
                Hist::SyncStart { ts, .. } => ("SStart", ts, "c/e", "1"),
                Hist::SyncStop { ts, .. } => ("SStop", ts, "c/e", "1"),
            };
            *counts.entry(kind.to_string()).or_insert(0) += 1;
            *counts.entry(ebuild.to_string()).or_insert(0) += 1;
            assert!(ts >= filter_mints.unwrap_or(mints) && ts <= filter_maxts.unwrap_or(maxts),
                    "Out of bound date {} in  in {p:?}",
                    fmt_utctime(ts));
            assert!(re_atom.is_match(ebuild), "Invalid ebuild atom {} in {p:?}", ebuild);
            assert!(re_version.is_match(version), "Invalid version {} in {p:?}", version);
        }
        // Check that we got the right number of each kind
        for (t, ref c) in expect_counts {
            let v = counts.get(t).unwrap_or(&0);
            assert_eq!(v, c,
                       "Got {} {}, expected {:?} with pkg={:?} exact={} min={:?} max={:?}",
                       v, t, c, filter_terms, exact, filter_mints, filter_maxts);
        }
    }

    #[test]
    /// Simplified emerge log containing all the ebuilds in all the versions of the current portage tree (see test/generate.sh)
    fn parse_hist_all() {
        let t = vec![("MStart", 31467)];
        chk_hist("all", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_nullbytes() {
        let t = vec![("MStart", 14), ("MStop", 14)];
        chk_hist("nullbytes", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badtimestamp() {
        let t = vec![("MStart", 2),
                     ("MStop", 3),
                     ("media-libs/jpeg", 1), //letter in timestamp
                     ("dev-libs/libical", 2),
                     ("media-libs/libpng", 2)];
        chk_hist("badtimestamp", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badversion() {
        let t = vec![("MStart", 3),
                     ("MStop", 2),
                     ("media-libs/jpeg", 2),
                     ("dev-libs/libical", 2),
                     ("media-libs/libpng", 1)]; //missing version
        chk_hist("badversion", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_shortline() {
        let t = vec![("MStart", 3),
                     ("MStop", 2),
                     ("media-libs/jpeg", 2),
                     ("dev-libs/libical", 1), //missing end of line and spaces in iter
                     ("media-libs/libpng", 2)];
        chk_hist("shortline", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Basic counts, with every combination of command/merge/unmerge/sync
    fn parse_hist_nofilter() {
        for i in 0..16 {
            let r = (i & 0b0001) == 0;
            let m = (i & 0b0010) == 0;
            let u = (i & 0b0100) == 0;
            let s = (i & 0b1000) == 0;
            let show = format!("{}{}{}{}",
                               if r { "r" } else { "" },
                               if m { "m" } else { "" },
                               if u { "u" } else { "" },
                               if s { "s" } else { "" });
            let t = vec![("RStart", if r { 451 } else { 0 }),
                         ("MStart", if m { 890 } else { 0 }),
                         ("MBin", if m { 1 } else { 0 }),
                         ("MStop", if m { 833 } else { 0 }),
                         ("UStart", if u { 833 } else { 0 }),
                         ("UStop", if u { 833 } else { 0 }),
                         ("SStart", if s { 326 } else { 0 }),
                         ("SStop", if s { 150 } else { 0 })];
            chk_hist("10000", &show, None, None, vec![], false, t);
        }
    }

    #[test]
    /// Filtering by search term
    fn parse_hist_filter_term() {
        #[rustfmt::skip]
        let t = vec![
            ("",                           false, 890,  1, 833, 833, 833, 150), // Everything
            ("kactivities",                false,   4,  0,   4,   4,   4,   0), // regexp matches 4
            ("kactivities",                true,    2,  0,   2,   2,   2,   0), // string matches 2
            ("kde-frameworks/kactivities", true,    2,  0,   2,   2,   2,   0), // string matches 2
            ("frameworks/kactivities",     true,    0,  0,   0,   0,   0,   0), // string matches nothing
            ("ks/kw",                      false,   9,  0,   8,   8,   8,   0), // regexp matches 16 (+1 failed)
            ("file",                       false,   7,  0,   7,   6,   6,   0), // case-insensitive
            ("FILE",                       false,   7,  0,   7,   6,   6,   0), // case-insensitive
            ("file-next",                  true,    0,  0,   0,   0,   0,   0), // case-sensitive
            ("File-Next",                  true,    1,  0,   1,   0,   0,   0), // case-sensitive
            ("gentoo",                     true,    0,  0,   0,   0,   0, 150), // repo sync only
            ("gentoo",                     false,  11,  0,  11,  12,  12, 150), // repo and ebuilds
            ("ark oxygen",                 false,  15,  0,  15,  15,  15,   0), // multiple regex terms
            ("ark oxygen",                 true,    8,  0,   8,   8,   8,   0), // multiple string terms
        ];
        for (f, e, m1, m2, m3, u1, u2, s2) in t {
            let c = vec![("MStart", m1),
                         ("MBin", m2),
                         ("MStop", m3),
                         ("UStart", u1),
                         ("UStop", u2),
                         // SStart is always the same because Sync filtering is only done for SStop
                         ("SStart", 326),
                         ("SStop", s2)];
            let terms = f.split_whitespace().map(str::to_string).collect();
            chk_hist("10000", "mus", None, None, terms, e, c);
        }
    }

    #[test]
    /// Filtering by timestamp
    fn parse_hist_filter_ts() {
        let (umin, umax, fmin, fmax) = (i64::MIN, i64::MAX, 1517609348, 1520991402);
        #[rustfmt::skip]
        let t = vec![(Some(umin),       None,           890,   1, 833, 833, 833, 326, 150),
                     (Some(fmin),       None,           890,   1, 833, 833, 833, 326, 150),
                     (None,             Some(umax),     890,   1, 833, 833, 833, 326, 150),
                     (None,             Some(fmax),     890,   1, 833, 833, 833, 326, 150),
                     (Some(fmin),       Some(fmax),     890,   1, 833, 833, 833, 326, 150),
                     (Some(fmax),       None,             0,   0,   1,   0,   0,   0,   0), //last ts contains a stop and a "Post-Build Cleaning" step
                     (None,             Some(fmin),       0,   0,   1,   0,   0,   0,   0), //fist ts contains a stop
                     (None,             Some(umin),       0,   0,   0,   0,   0,   0,   0),
                     (Some(umax),       None,             0,   0,   0,   0,   0,   0,   0),
                     (Some(1517917751), Some(1517931835), 6,   0,   6,   5,   5,   4,   2),
                     (Some(1517959010), Some(1518176159), 24,  0,  21,  23,  23,  32,  16),
        ];
        for (min, max, m1, m2, m3, u1, u2, s1, s2) in t {
            let c = vec![("MStart", m1),
                         ("MBin", m2),
                         ("MStop", m3),
                         ("UStart", u1),
                         ("UStop", u2),
                         ("SStart", s1),
                         ("SStop", s2)];
            chk_hist("10000", "mus", min, max, vec![], true, c);
        }
    }

    #[test]
    /// Filtering by search term
    fn filter_terms() {
        let t = vec![("a", true, "a", false, true),
                     ("a", true, "b/a", true, false),
                     ("a", true, "aa", false, false),
                     ("a", true, "b/aa", false, false),
                     ("a.", true, "ab", false, false),
                     ("a.", false, "ab", true, true),];
        for (terms, e, s, mpkg, mstr) in t {
            let t: Vec<String> = terms.split_whitespace().map(str::to_string).collect();
            let f = FilterStr::try_new(&t, e).unwrap();
            assert_eq!(f.match_pkg(s), mpkg, "filter({t:?}, {e}).match_pkg({s:?})");
            assert_eq!(f.match_str(s), mstr, "filter({t:?}, {e}).match_str({s:?})");
        }
    }

    #[test]
    fn split_atom() {
        let g = |s| parse_version(s, &FilterStr::True).map(|n| (&s[..n - 1], &s[n..]));
        assert_eq!(None, g(""));
        assert_eq!(None, g("a"));
        assert_eq!(None, g("-"));
        assert_eq!(None, g("42"));
        assert_eq!(None, g("-42"));
        assert_eq!(None, g("42-"));
        assert_eq!(None, g("a-/"));
        assert_eq!(Some(("a", "0")), g("a-0"));
        assert_eq!(Some(("a", "1")), g("a-1"));
        assert_eq!(Some(("a", "2")), g("a-2"));
        assert_eq!(Some(("a", "3")), g("a-3"));
        assert_eq!(Some(("a", "4")), g("a-4"));
        assert_eq!(Some(("a", "5")), g("a-5"));
        assert_eq!(Some(("a", "6")), g("a-6"));
        assert_eq!(Some(("a", "7")), g("a-7"));
        assert_eq!(Some(("a", "8")), g("a-8"));
        assert_eq!(Some(("a", "9")), g("a-9"));
        assert_eq!(None, g("a-:"));
        assert_eq!(Some(("a-b", "2")), g("a-b-2"));
        assert_eq!(Some(("a-b", "2-3")), g("a-b-2-3"));
        assert_eq!(Some(("a-b", "2-3_r1")), g("a-b-2-3_r1"));
        assert_eq!(Some(("a-b", "2foo-4")), g("a-b-2foo-4"));
        assert_eq!(Some(("a-b", "2foo-4-")), g("a-b-2foo-4-"));
        assert_eq!(Some(("Noël", "2-bêta")), g("Noël-2-bêta"));
    }
}

#[cfg(feature = "unstable")]
#[cfg(test)]
mod bench {
    use super::*;
    use crate::config::*;
    use std::sync::LazyLock;
    extern crate test;

    static EMERGE_LOG: &str = include_str!("../../benches/emerge.log");

    /// Vec<(full, no_ts)> of emerge.log lines
    static EMERGE_LINES: LazyLock<Vec<(&[u8], &[u8])>> = LazyLock::new(|| {
        EMERGE_LOG.lines()
                  .map(|l| {
                      let mut p = l.find(' ').unwrap();
                      while l.as_bytes().get(p) == Some(&b' ') {
                          p += 1;
                      }
                      (l.as_bytes(), l[p..].as_bytes())
                  })
                  .collect()
    });

    /// Vec<String> of package categ/name-version
    static PKGS: LazyLock<Vec<String>> = LazyLock::new(|| {
        let f = |p| match p {
            Hist::MergeStart { key, .. } => Some(key),
            _ => None,
        };
        let show = Show::parse(&String::from("ms"), "rptsmua", "test").unwrap();
        let file = String::from("benches/emerge.log");
        let tb = TimeBound::None;
        let pkgs: Vec<_> =
            get_hist(&file, tb, tb, show, &vec![], true).unwrap().iter().filter_map(f).collect();
        assert_eq!(pkgs.len(), 10790);
        pkgs
    });

    macro_rules! bench_filterstr {
        ($n:ident, $t:expr, $e:expr) => {
            #[bench]
            /// Bench creating a filter and applying it on many strings
            fn $n(b: &mut test::Bencher) {
                let t: Vec<String> = $t.split_whitespace().map(str::to_string).collect();
                let pkgs = &*PKGS;
                b.iter(move || {
                     let f = FilterStr::try_new(&t, $e).unwrap();
                     pkgs.iter().fold(true, |a, p| a ^ f.match_pkg(&p))
                 });
            }
        };
    }
    bench_filterstr!(filterstr_none, "", true);
    bench_filterstr!(filterstr_one_str, "gcc", true);
    bench_filterstr!(filterstr_one_full, "virtual/rust", true);
    bench_filterstr!(filterstr_many_str, "gcc llvm clang rust emacs", true);
    bench_filterstr!(filterstr_one_reg, "gcc", false);
    bench_filterstr!(filterstr_many_reg, "gcc llvm clang rust emacs", false);

    #[bench]
    fn parse_version_(b: &mut test::Bencher) {
        let pkgs = &*PKGS;
        b.iter(move || {
             for p in pkgs {
                 parse_version(&p, &FilterStr::True);
             }
         });
    }

    /// Bench parsing a whole log file without filter or postprocessing
    fn get_hist_with(b: &mut test::Bencher, s: &str) {
        let show = Show::parse(&String::from(s), "rptsmua", "test").unwrap();
        let count: usize = s.chars()
                            .map(|c| match c {
                                'm' => 21320,
                                'u' => 20847,
                                's' => 661,
                                'r' => 971,
                                o => panic!("unhandled show {o}"),
                            })
                            .sum();
        let file = String::from("benches/emerge.log");
        b.iter(move || {
             let mut n = 0;
             let hist =
                 get_hist(&file, TimeBound::None, TimeBound::None, show, &vec![], true).unwrap();
             for _ in hist {
                 n += 1;
             }
             assert_eq!(n, count);
         });
    }
    #[bench]
    fn get_hist_murs(b: &mut test::Bencher) {
        get_hist_with(b, "murs")
    }
    #[bench]
    fn get_hist_m(b: &mut test::Bencher) {
        get_hist_with(b, "m")
    }
    #[bench]
    fn get_hist_u(b: &mut test::Bencher) {
        get_hist_with(b, "u")
    }
    #[bench]
    fn get_hist_r(b: &mut test::Bencher) {
        get_hist_with(b, "r")
    }
    #[bench]
    fn get_hist_s(b: &mut test::Bencher) {
        get_hist_with(b, "s")
    }

    macro_rules! bench_parselines {
        ($n:ident, $f:expr) => {
            #[bench]
            fn $n(b: &mut test::Bencher) {
                let f = FilterStr::True;
                let lines = &*EMERGE_LINES;
                b.iter(move || {
                     let mut found = 0;
                     for (l1, l2) in lines {
                         found += $f(&f, l1, l2).is_some() as u32;
                     }
                     assert!(found > 2, "Only {found} matches for {}", stringify!($f));
                 });
            }
        };
    }
    bench_parselines!(parse_ts_, |_, s, _| parse_ts(s, i64::MIN, i64::MAX));
    bench_parselines!(parse_runstart_, |_, _, s| parse_runstart(true, 1, s));
    bench_parselines!(parse_mergestart_, |f, _, s| parse_mergestart(true, 1, s, f));
    bench_parselines!(parse_mergestop_, |f, _, s| parse_mergestop(true, 1, s, f));
    bench_parselines!(parse_mergebin_, |f, _, s| parse_mergebin(true, 1, s, f));
    bench_parselines!(parse_unmergestart_, |f, _, s| parse_unmergestart(true, 1, s, f));
    bench_parselines!(parse_unmergestop_, |f, _, s| parse_unmergestop(true, 1, s, f));
    bench_parselines!(parse_syncstart_, |_, _, s| parse_syncstart(true, 1, s));
    bench_parselines!(parse_syncstop_, |f, _, s| parse_syncstop(true, 1, s, f));
}
