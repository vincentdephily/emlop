//! Handles emerge.log parsing.
//!
//! Use `new_hist()` to start parsing and retrieve `Hist` enums.

use crate::{datetime::fmt_utctime, Show};
use anyhow::{bail, ensure, Context, Error};
use crossbeam_channel::{bounded, Receiver, Sender};
use flate2::read::GzDecoder;
use log::*;
use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use std::{fs::File,
          io::{BufRead, BufReader},
          str::from_utf8,
          thread};

/// Items sent on the channel returned by `new_hist()`.
#[derive(Debug)]
pub enum Hist {
    /// Merge started (might never complete).
    MergeStart { ts: i64, key: String, pos: usize },
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
            Self::MergeStart { key, pos, .. } => &key[..(*pos - 1)],
            Self::MergeStop { key, pos, .. } => &key[..(*pos - 1)],
            Self::UnmergeStart { key, pos, .. } => &key[..(*pos - 1)],
            Self::UnmergeStop { key, pos, .. } => &key[..(*pos - 1)],
            _ => unreachable!("No ebuild for {:?}", self),
        }
    }
    pub fn version(&self) -> &str {
        match self {
            Self::MergeStart { key, pos, .. } => &key[*pos..],
            Self::MergeStop { key, pos, .. } => &key[*pos..],
            Self::UnmergeStart { key, pos, .. } => &key[*pos..],
            Self::UnmergeStop { key, pos, .. } => &key[*pos..],
            _ => unreachable!("No version for {:?}", self),
        }
    }
    pub fn ebuild_version(&self) -> &str {
        match self {
            Self::MergeStart { key, .. } => key,
            Self::MergeStop { key, .. } => key,
            Self::UnmergeStart { key, .. } => key,
            Self::UnmergeStop { key, .. } => key,
            _ => unreachable!("No ebuild/version for {:?}", self),
        }
    }
    pub fn ts(&self) -> i64 {
        match self {
            Self::MergeStart { ts, .. } => *ts,
            Self::MergeStop { ts, .. } => *ts,
            Self::UnmergeStart { ts, .. } => *ts,
            Self::UnmergeStop { ts, .. } => *ts,
            Self::SyncStart { ts, .. } => *ts,
            Self::SyncStop { ts, .. } => *ts,
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
pub fn get_hist(file: String,
                min_ts: Option<i64>,
                max_ts: Option<i64>,
                show: Show,
                search_terms: Vec<String>,
                search_exact: bool)
                -> Result<Receiver<Hist>, Error> {
    debug!("get_hist input={} min={:?} max={:?} str={:?} exact={}",
           file, min_ts, max_ts, search_terms, search_exact);
    let mut buf = open_any_buffered(&file)?;
    let (ts_min, ts_max) = filter_ts(min_ts, max_ts)?;
    let filter = FilterStr::try_new(search_terms, search_exact)?;
    let (tx, rx): (Sender<Hist>, Receiver<Hist>) = bounded(256);
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
                            warn!("{file}:{curline}: System clock jump: {} -> {}",
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
                        } else if let Some(found) = parse_syncstart(show.sync, t, s) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        } else if let Some(found) = parse_syncstop(show.sync, t, s, &filter) {
                            if tx.send(found).is_err() {
                                break;
                            }
                        }
                    }
                },
                // Could be invalid UTF8, system read error...
                Err(e) => warn!("{file}:{curline}: {e}"),
            }
            line.clear();
            curline += 1;
        }
    });
    Ok(rx)
}

/// Return min/max timestamp depending on options.
fn filter_ts(min: Option<i64>, max: Option<i64>) -> Result<(i64, i64), Error> {
    match (min, max) {
        (None, None) => debug!("Date filter: None"),
        (Some(a), None) => debug!("Date filter: after {}", fmt_utctime(a)),
        (None, Some(b)) => debug!("Date filter: before {}", fmt_utctime(b)),
        (Some(a), Some(b)) if a < b => {
            debug!("Date filter: between {} and {}", fmt_utctime(a), fmt_utctime(b))
        },
        (Some(a), Some(b)) => {
            bail!("Invalid date filter: {} <= {}, did you swap --to and --from ?",
                  fmt_utctime(a),
                  fmt_utctime(b))
        },
    }
    Ok((min.unwrap_or(std::i64::MIN), max.unwrap_or(std::i64::MAX)))
}

/// Matches package/repo depending on options.
enum FilterStr {
    True,
    Eq { a: Vec<String>, b: Vec<String>, c: Vec<String> },
    Re1 { r: Regex },
    Re { r: RegexSet },
}
impl FilterStr {
    fn try_new(terms: Vec<String>, exact: bool) -> Result<Self, regex::Error> {
        debug!("Term filter: {terms:?} {exact}");
        Ok(match (terms.len(), exact) {
            (0, _) => Self::True,
            (_, true) => {
                let (b, c) = terms.iter().cloned().partition(|s| s.contains('/'));
                Self::Eq { a: terms, b, c: c.into_iter().map(|s| format!("/{s}")).collect() }
            },
            (1, false) => {
                Self::Re1 { r: RegexBuilder::new(&terms[0]).case_insensitive(true).build()? }
            },
            (_, false) => {
                Self::Re { r: RegexSetBuilder::new(&terms).case_insensitive(true).build()? }
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
fn find_version(atom: &str, filter: &FilterStr) -> Option<usize> {
    let mut pos = 0;
    loop {
        pos += atom[pos..].find('-')?;
        if pos > 0 && atom.as_bytes().get(pos + 1)?.is_ascii_digit() {
            return filter.match_pkg(&atom[..pos]).then_some(pos + 1);
        }
        pos += 1;
    }
}

/// Parse and filter timestamp
// TODO from_utf8(s.trim_ascii_start()) https://github.com/rust-lang/rust/issues/94035
fn parse_ts(line: &[u8], min: i64, max: i64) -> Option<(i64, &[u8])> {
    use atoi::FromRadix10;
    match i64::from_radix_10(line) {
        (ts, n) if n != 0 && ts >= min && ts <= max => {
            let mut line = &line[(n + 1)..];
            while let Some(32) = line.first() {
                line = &line[1..];
            }
            Some((ts, line))
        },
        _ => None,
    }
}

fn parse_mergestart(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b">>> emer") {
        return None;
    }
    let mut tokens = from_utf8(line).ok()?.split_ascii_whitespace();
    let t6 = tokens.nth(5)?;
    let pos = find_version(t6, filter)?;
    Some(Hist::MergeStart { ts, key: t6.to_owned(), pos })
}

fn parse_mergestop(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b"::: comp") {
        return None;
    }
    let mut tokens = from_utf8(line).ok()?.split_ascii_whitespace();
    let t7 = tokens.nth(6)?;
    let pos = find_version(t7, filter)?;
    Some(Hist::MergeStop { ts, key: t7.to_owned(), pos })
}

fn parse_unmergestart(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b"=== Unmerging...") {
        return None;
    }
    let mut tokens = from_utf8(line).ok()?.split_ascii_whitespace();
    let t3 = tokens.nth(2)?;
    let ebuild_version = &t3[1..t3.len() - 1];
    let pos = find_version(ebuild_version, filter)?;
    Some(Hist::UnmergeStart { ts, key: ebuild_version.to_owned(), pos })
}

fn parse_unmergestop(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    if !enabled || !line.starts_with(b">>> unmerge success") {
        return None;
    }
    let mut tokens = from_utf8(line).ok()?.split_ascii_whitespace();
    let t3 = tokens.nth(3)?;
    let pos = find_version(t3, filter)?;
    Some(Hist::UnmergeStop { ts, key: t3.to_owned(), pos })
}

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
fn parse_syncstop(enabled: bool, ts: i64, line: &[u8], filter: &FilterStr) -> Option<Hist> {
    // Old portage logs 'completed with <url>', new portage logs 'completed for <name>'
    if !enabled || !line.starts_with(b"=== Sync completed") {
        return None;
    }
    let line = from_utf8(line).ok()?;
    let repo = match line.rsplit_once(['/', ' ']) {
        Some((_, r)) => String::from(r.trim()),
        _ => {
            warn!("Can't find sync repo name in {ts} {line}");
            String::from("unknown")
        },
    };
    if !filter.match_str(&repo) {
        return None;
    }
    Some(Hist::SyncStop { ts, repo })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// This checks parsing the given emerge.log.
    fn chk_hist(file: &str,
                parse_merge: bool,
                parse_unmerge: bool,
                parse_sync: bool,
                filter_mints: Option<i64>,
                filter_maxts: Option<i64>,
                filter_terms: Vec<String>,
                exact: bool,
                expect_counts: Vec<(&str, usize)>) {
        // Setup
        let (mints, maxts) = match file {
            "10000" => (1517609348, 1520891098),
            "all" => (1483228800, 1483747200),
            "badtimestamp" => (1327867709, 1327871057),
            "badversion" => (1327867709, 1327871057),
            "nullbytes" => (1327867709, 1327871057),
            "shortline" => (1327867709, 1327871057),
            o => unimplemented!("Unknown test log file {:?}", o),
        };
        let hist = get_hist(format!("tests/emerge.{}.log", file),
                            filter_mints,
                            filter_maxts,
                            Show { merge: parse_merge,
                                   unmerge: parse_unmerge,
                                   sync: parse_sync,
                                   ..Show::default() },
                            filter_terms.clone(),
                            exact).unwrap();
        let re_atom = Regex::new("^[a-zA-Z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap();
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap();
        let mut counts: HashMap<String, usize> = HashMap::new();
        // Check that all items look valid
        for p in hist {
            let (kind, ts, ebuild, version) = match p {
                Hist::MergeStart { ts, .. } => ("MStart", ts, p.ebuild(), p.version()),
                Hist::MergeStop { ts, .. } => ("MStop", ts, p.ebuild(), p.version()),
                Hist::UnmergeStart { ts, .. } => ("UStart", ts, p.ebuild(), p.version()),
                Hist::UnmergeStop { ts, .. } => ("UStop", ts, p.ebuild(), p.version()),
                Hist::SyncStart { ts, .. } => ("SStart", ts, "c/e", "1"),
                Hist::SyncStop { ts, .. } => ("SStop", ts, "c/e", "1"),
            };
            *counts.entry(kind.to_string()).or_insert(0) += 1;
            *counts.entry(ebuild.to_string()).or_insert(0) += 1;
            assert!(ts >= filter_mints.unwrap_or(mints) && ts <= filter_maxts.unwrap_or(maxts),
                    "Out of bound date {}",
                    fmt_utctime(ts));
            assert!(re_atom.is_match(ebuild), "Invalid ebuild atom {}", ebuild);
            assert!(re_version.is_match(version), "Invalid version {}", version);
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
        chk_hist("all", true, false, false, None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_nullbytes() {
        let t = vec![("MStart", 14), ("MStop", 14)];
        chk_hist("nullbytes", true, false, false, None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badtimestamp() {
        let t = vec![("MStart", 2),
                     ("MStop", 3),
                     ("media-libs/jpeg", 1), //letter in timestamp
                     ("dev-libs/libical", 2),
                     ("media-libs/libpng", 2)];
        chk_hist("badtimestamp", true, false, false, None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badversion() {
        let t = vec![("MStart", 3),
                     ("MStop", 2),
                     ("media-libs/jpeg", 2),
                     ("dev-libs/libical", 2),
                     ("media-libs/libpng", 1)]; //missing version
        chk_hist("badversion", true, false, false, None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_shortline() {
        let t = vec![("MStart", 3),
                     ("MStop", 2),
                     ("media-libs/jpeg", 2),
                     ("dev-libs/libical", 1), //missing end of line and spaces in iter
                     ("media-libs/libpng", 2)];
        chk_hist("shortline", true, false, false, None, None, vec![], false, t);
    }

    #[test]
    /// Basic counts, with every combination of merge/unmerge/sync
    fn parse_hist_nofilter() {
        for i in 0..8 {
            let m = (i & 0b001) == 0;
            let u = (i & 0b010) == 0;
            let s = (i & 0b100) == 0;
            let t = vec![("MStart", if m { 889 } else { 0 }),
                         ("MStop", if m { 832 } else { 0 }),
                         ("UStart", if u { 832 } else { 0 }),
                         ("UStop", if u { 832 } else { 0 }),
                         ("SStart", if s { 326 } else { 0 }),
                         ("SStop", if s { 150 } else { 0 })];
            chk_hist("10000", m, u, s, None, None, vec![], false, t);
        }
    }

    #[test]
    /// Filtering by search term
    fn parse_hist_filter_term() {
        #[rustfmt::skip]
        let t = vec![("",                           false, 889, 832, 832, 832, 150), // Everything
                     ("kactivities",                false, 4, 4, 4, 4, 0), // regexp matches 4
                     ("kactivities",                true,  2, 2, 2, 2, 0), // string matches 2
                     ("kde-frameworks/kactivities", true,  2, 2, 2, 2, 0), // string matches 2
                     ("frameworks/kactivities",     true,  0, 0, 0, 0, 0), // string matches nothing
                     ("ks/kw",                      false, 9, 8, 8, 8, 0), // regexp matches 16 (+1 failed)
                     ("file",                       false, 7, 7, 6, 6, 0), // case-insensitive
                     ("FILE",                       false, 7, 7, 6, 6, 0), // case-insensitive
                     ("file-next",                  true,  0, 0, 0, 0, 0), // case-sensitive
                     ("File-Next",                  true,  1, 1, 0, 0, 0), // case-sensitive
                     ("gentoo",                     true,  0, 0, 0, 0, 150), // repo sync only
                     ("gentoo",                     false, 11, 11, 12, 12, 150), // repo and ebuilds
                     ("ark oxygen",                 false, 15, 15, 15, 15, 0), // multiple regex terms
                     ("ark oxygen",                 true,  8, 8, 8, 8, 0), // multiple string terms
        ];
        for (f, e, m1, m2, u1, u2, s2) in t {
            let c = vec![("MStart", m1),
                         ("MStop", m2),
                         ("UStart", u1),
                         ("UStop", u2),
                         // SStart is always the same because Sync filtering is only done for SStop
                         ("SStart", 326),
                         ("SStop", s2)];
            let terms = f.split_whitespace().map(str::to_string).collect();
            chk_hist("10000", true, true, true, None, None, terms, e, c);
        }
    }

    #[test]
    /// Filtering by timestamp
    fn parse_hist_filter_ts() {
        let (umin, umax, fmin, fmax) = (std::i64::MIN, std::i64::MAX, 1517609348, 1520891098);
        #[rustfmt::skip]
        let t = vec![(Some(umin),       None,           889, 832, 832, 832, 326, 150),
                     (Some(fmin),       None,           889, 832, 832, 832, 326, 150),
                     (None,             Some(umax),     889, 832, 832, 832, 326, 150),
                     (None,             Some(fmax),     889, 832, 832, 832, 326, 150),
                     (Some(fmin),       Some(fmax),     889, 832, 832, 832, 326, 150),
                     (Some(fmax),       None,             0,   0,   0,   0,   0,   0),
                     (None,             Some(fmin),       0,   1,   0,   0,   0,   0), //fist line of this file happens to be a stop
                     (None,             Some(umin),       0,   0,   0,   0,   0,   0),
                     (Some(umax),       None,             0,   0,   0,   0,   0,   0),
                     (Some(1517917751), Some(1517931835), 6,   6,   5,   5,   4,   2),
                     (Some(1517959010), Some(1518176159), 24, 21,  23,  23,  32,  16),
        ];
        for (min, max, m1, m2, u1, u2, s1, s2) in t {
            let c = vec![("MStart", m1),
                         ("MStop", m2),
                         ("UStart", u1),
                         ("UStop", u2),
                         ("SStart", s1),
                         ("SStop", s2)];
            chk_hist("10000", true, true, true, min, max, vec![], true, c);
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
            let f = FilterStr::try_new(t.clone(), e).unwrap();
            assert_eq!(f.match_pkg(s), mpkg, "filter({t:?}, {e}).match_pkg({s:?})");
            assert_eq!(f.match_str(s), mstr, "filter({t:?}, {e}).match_str({s:?})");
        }
    }

    #[test]
    fn split_atom() {
        let f = FilterStr::try_new(vec![], false).unwrap();
        let g = |s| find_version(s, &f).map(|n| (&s[..n - 1], &s[n..]));
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
    extern crate test;

    fn pkgs() -> Vec<String> {
        let f = |p| match p {
            Hist::MergeStart { key, .. } => key,
            Hist::SyncStop { repo, .. } => repo,
            _ => String::from("other"),
        };
        let show = Show { merge: true, sync: true, ..Show::default() };
        let file = String::from("benches/emerge.log");
        let pkgs: Vec<_> =
            get_hist(file, None, None, show, vec![], true).unwrap().iter().map(f).collect();
        assert_eq!(pkgs.len(), 21963);
        pkgs
    }

    macro_rules! bench_filterstr {
        ($n:ident, $t:expr, $e:expr) => {
            #[bench]
            /// Bench creating a filter and applying it on many strings
            fn $n(b: &mut test::Bencher) {
                let p = pkgs();
                let t: Vec<String> = $t.split_whitespace().map(str::to_string).collect();
                b.iter(move || {
                     let f = FilterStr::try_new(t.clone(), $e).unwrap();
                     p.iter().fold(true, |a, p| a ^ f.match_pkg(&p))
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
}
