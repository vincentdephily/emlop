//! Handles the actual log parsing.
//!
//! Instantiate a `Parser` and iterate over it to retrieve the events.

use crate::fmt_time;

use crossbeam_channel::{unbounded, Receiver, Sender};
use failure::Error;
use regex::{Regex, RegexBuilder};
use std;
use std::{io::{BufRead, BufReader, Read}, thread};

/// Items sent on the channel returned by `new_hist()`.
#[derive(Debug)]
pub enum ParsedHist {
    /// Merge started (might never complete).
    Start{ts: i64, ebuild: String, version: String, iter: String},
    /// Merge completed.
    Stop{ts: i64, ebuild: String, version: String, iter: String},
    /// Sync started (might never complete).
    SyncStart{ts: i64},
    /// Sync completed.
    SyncStop{ts: i64},
}

/// Items sent on the channel returned by `new_pretend()`.
#[derive(Debug)]
pub struct ParsedPretend{pub ebuild: String, pub version: String}

/// Parse emerge log into a channel of `Parsed` enums.
pub fn new_hist<R: Read>(reader: R, filename: String,
                         min_ts: Option<i64>, max_ts: Option<i64>,
                         parse_merge: bool, parse_sync: bool,
                         search_str: Option<&str>, search_exact: bool)
                         -> Result<Receiver<ParsedHist>, Error> where R: std::marker::Send+'static {
    debug!("new_hist input={} min={:?} max={:?} str={:?} exact={}", filename, min_ts, max_ts, search_str, search_exact);
    let (tx, rx): (Sender<ParsedHist>, Receiver<ParsedHist>) = unbounded();
    let filter_ts = filter_ts_fn(min_ts, max_ts);
    let filter_pkg = filter_pkg_fn(search_str, search_exact)?;
    thread::spawn(move || {
        for (curline,l) in BufReader::new(reader).lines().enumerate() {
            match l {
                Ok(ref line) => { // Got a line, see if one of the funs match it
                    if let Some((t,s)) = parse_ts(line, &filter_ts) {
                        if let Some(found) = parse_start(parse_merge, t, s, &filter_pkg) {tx.send(found)}
                        else if let Some(found) = parse_stop(parse_merge, t, s, &filter_pkg) {tx.send(found)}
                        else if let Some(found) = parse_syncstart(parse_sync, t, s) {tx.send(found)}
                        else if let Some(found) = parse_syncstop(parse_sync, t, s) {tx.send(found)}
                    }
                },
                Err(e) => // Could be invalid UTF8, system read error...
                    warn!("{}:{}: {}", filename, curline, e),
            }
        }
    });
    Ok(rx)
}

/// Parse portage pretend output into a Vec of `Parsed` enums.
pub fn new_pretend<R: Read>(reader: R, filename: &str) -> Vec<ParsedPretend> where R: std::marker::Send+'static {
    debug!("new_pretend input={}", filename);
    let mut out: Vec<ParsedPretend> = vec![];
    let re = Regex::new("^\\[ebuild[^]]+\\] (.+?)-([0-9][0-9a-z._-]*)").unwrap();
    for (curline,l) in BufReader::new(reader).lines().enumerate() {
        match l {
            Ok(ref line) => { // Got a line, see if one of the funs match it
                if let Some(found) = parse_pretend(line, &re) {out.push(found)}
            },
            Err(e) => // Could be invalid UTF8, system read error...
                warn!("{}:{}: {}", filename, curline, e),
        }
    }
    out
}


/// Create a closure that matches timestamp depending on options.
fn filter_ts_fn(min: Option<i64>, max: Option<i64>) -> impl Fn(i64) -> bool {
    match (min, max) {
        (None,    None) =>    info!("Date filter: None"),
        (Some(a), None) =>    info!("Date filter: after {}", fmt_time(a)),
        (None,    Some(b)) => info!("Date filter: before {}", fmt_time(b)),
        (Some(a), Some(b)) => info!("Date filter: between {} and {}", fmt_time(a), fmt_time(b)),
    }
    let mi = min.unwrap_or(std::i64::MIN);
    let ma = max.unwrap_or(std::i64::MAX);
    move |n| { n >= mi && n <= ma }
}

/// Create a closure that matches package depending on options.
fn filter_pkg_fn(package: Option<&str>, exact: bool) -> Result<impl Fn(&str) -> bool, Error> {
    enum FilterPkg {
        True,
        Eq{e: String},
        Ends{e: String},
        Re{r: Regex},
    }
    let fp = match (&package, exact, package.as_ref().map_or(false, |p| p.contains('/'))) {
        (None, _, _) => {
            info!("Package filter: None");
            FilterPkg::True
        },
        (Some(search), true, true) => {
            info!("Package filter: categ/name == {}", search);
            FilterPkg::Eq{e:search.to_string()}
        },
        (Some(search), true, false) => {
            info!("Package filter: name == {}", search);
            FilterPkg::Ends{e:format!("/{}",search)}
        },
        (Some(search), false, _) => {
            info!("Package filter: categ/name ~= {}", search);
            FilterPkg::Re{r:RegexBuilder::new(&search)
                          .case_insensitive(true)
                          .build()?}
        }
    };
    Ok(move |s: &str| match &fp {
        FilterPkg::True => true,
        FilterPkg::Eq{e} => e == s,
        FilterPkg::Ends{e} => s.ends_with(e),
        FilterPkg::Re{r} => r.is_match(s),
    })
}

/// Split "categ/name-version" into "categ/name" and "version"
fn split_atom(atom: &str) -> Option<(&str, &str)> {
    let mut start = 0;
    loop {
        let pos = atom[start..].find('-')?;
        if atom.len() <= start+pos+1 {return None}
        if atom.as_bytes()[start+pos+1].is_ascii_digit() && pos > 0 {
            return Some((&atom[..start+pos], &atom[start+pos+1..]))
        }
        start += if pos==0 {1} else {pos};
    }
}

fn parse_ts(line: &str, filter_ts: impl Fn(i64) -> bool) -> Option<(i64,&str)> {
    let (ts_str,rest) = line.split_at(line.find(':')?);
    let ts = ts_str.parse::<i64>().ok()?;
    if !(filter_ts)(ts) {return None}
    Some((ts,&rest[2..]))
}
fn parse_start(enabled: bool, ts: i64, line: &str, filter_pkg: impl Fn(&str) -> bool) -> Option<ParsedHist> {
    if !enabled || !line.starts_with(" >>> emer") {return None}
    let mut tokens = line.split_whitespace(); // https://github.com/rust-lang/rust/issues/48656
    let (t3,t5,t6) = (tokens.nth(2)?, tokens.nth(1)?, tokens.nth(0)?);
    let (ebuild,version) = split_atom(t6)?;
    if !(filter_pkg)(ebuild) {return None}
    Some(ParsedHist::Start{ts,
                           ebuild: ebuild.to_string(),
                           iter: format!("{} {}", t3, t5),
                           version: version.to_string()})
}
fn parse_stop(enabled: bool, ts: i64, line: &str, filter_pkg: impl Fn(&str) -> bool) -> Option<ParsedHist> {
    if !enabled || !line.starts_with(" ::: comp") {return None}
    let mut tokens = line.split_whitespace();
    let (t4,t6,t7) = (tokens.nth(3)?, tokens.nth(1)?, tokens.nth(0)?);
    let (ebuild,version) = split_atom(t7)?;
    if !(filter_pkg)(ebuild) {return None}
    Some(ParsedHist::Stop{ts,
                          ebuild: ebuild.to_string(),
                          iter: format!("{} {}", t4, t6),
                          version: version.to_string()})
}
fn parse_syncstart(enabled: bool, ts: i64, line: &str) -> Option<ParsedHist> {
    if !enabled || line != " === sync" {return None}
    Some(ParsedHist::SyncStart{ts})
}
fn parse_syncstop(enabled: bool, ts: i64, line: &str) -> Option<ParsedHist> {
    // Old portage logs 'completed with <source>', new portage logs 'completed for <destination>'
    if !enabled || !line.starts_with("=== Sync completed") {return None}
    Some(ParsedHist::SyncStop{ts})
}
fn parse_pretend(line: &str, re: &Regex) -> Option<ParsedPretend> {
    let c = re.captures(line)?;
    Some(ParsedPretend{ebuild: c.get(1).unwrap().as_str().to_string(),
                       version: c.get(2).unwrap().as_str().to_string()})
}


#[cfg(test)]
mod tests {
    use crate::parser::*;
    use std::collections::HashMap;
    use std::fs::File;

    /// This checks parsing the given emerge.log.
    fn parse_hist(filename: &str, mints: i64, maxts: i64,
                  filter_mints: Option<i64>, filter_maxts: Option<i64>,
                  parse_merge: bool, parse_sync: bool,
                  filter_pkg: Option<&str>, exact: bool,
                  expect_counts: Vec<(&str, usize)>) {
        // Setup
        let hist = new_hist(File::open(filename).unwrap(), filename.into(), filter_mints, filter_maxts, parse_merge, parse_sync, filter_pkg, exact).unwrap();
        let re_atom = Regex::new("^[a-z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap(); //FIXME use catname.txt
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap(); //Should match pattern used in *Parser
        let re_iter = Regex::new("^\\([1-9][0-9]* [1-9][0-9]*\\)$").unwrap(); //Should match pattern used in *Parser
        let mut counts: HashMap<String, usize> = HashMap::new();
        // Check that all items look valid
        for p in hist {
            let (kind, ts, ebuild, version, iter) = match p {
                ParsedHist::Start{ts, ebuild, version, iter} => ("start", ts, ebuild, version, iter),
                ParsedHist::Stop{ts, ebuild, version, iter} =>  ("stop",  ts, ebuild, version, iter),
                ParsedHist::SyncStart{ts} => ("syncstart", ts, "c/e".into(), "1".into(), "(1 1)".into()),
                ParsedHist::SyncStop{ts} => ("syncstop", ts, "c/e".into(), "1".into(), "(1 1)".into()),
            };
            *counts.entry(kind.to_string()).or_insert(0) += 1;
            *counts.entry(ebuild.clone()).or_insert(0) += 1;
            assert!(ts >= filter_mints.unwrap_or(mints) && ts <= filter_maxts.unwrap_or(maxts), "Out of bound date {}", fmt_time(ts));
            assert!(re_atom.is_match(&ebuild), "Invalid ebuild atom {}", ebuild);
            assert!(re_version.is_match(&version), "Invalid version {}", version);
            assert!(re_iter.is_match(&iter), "Invalid iteration {}", iter);
        }
        // Check that we got the right number of each kind
        for (t, ref c) in expect_counts {
            let v = counts.get(t).unwrap_or(&0);
            assert_eq!(v, c, "Got {} {}, expected {:?} with {:?} {} {:?} {:?}", v, t, c, filter_pkg, exact, filter_mints, filter_maxts);
        }
    }

    #[test]
    /// Simplified emerge log containing all the ebuilds in all the versions of the current portage tree (see test/generate.sh)
    fn parse_hist_all() {
        parse_hist("test/emerge.all.log", 1483228800, 1483747200,
                   None, None, true, false, None, false,
                   vec![("start",37415),("stop",37415)]);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_nullbytes() {
        parse_hist("test/emerge.nullbytes.log", 1327867709, 1327871057,
                   None, None, true, false, None, false,
                   vec![("start",14),("stop",14)]);
    }
    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badtimestamp() {
        parse_hist("test/emerge.badtimestamp.log", 1327867709, 1327871057,
                   None, None, true, false, None, false,
                   vec![("start",2),("stop",3),
                        ("media-libs/jpeg",1),    //letter in timestamp
                        ("dev-libs/libical",2),
                        ("media-libs/libpng",2)]);
    }
    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badversion() {
        parse_hist("test/emerge.badversion.log", 1327867709, 1327871057,
                   None, None, true, false, None, false,
                   vec![("start",3),("stop",2),
                        ("media-libs/jpeg",2),
                        ("dev-libs/libical",2),
                        ("media-libs/libpng",1)]); //missing version
    }
    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_shortline() {
        parse_hist("test/emerge.shortline.log", 1327867709, 1327871057,
                   None, None, true, false, None, false,
                   vec![("start",3),("stop",2),
                        ("media-libs/jpeg",2),
                        ("dev-libs/libical",1),    //missing end of line and spaces in iter
                        ("media-libs/libpng",2)]);
    }

    #[test]
    /// Filtering by package
    fn parse_hist_filter_pkg() {
        for (f,e,c1,c2) in vec![(None,                               false, 889, 832), // Everything
                                (Some("kactivities"),                false,   4,   4), // regexp matches 4
                                (Some("kactivities"),                true,    2,   2), // string matches 2
                                (Some("kde-frameworks/kactivities"), true,    2,   2), // string matches 2
                                (Some("frameworks/kactivities"),     true,    0,   0), // string matches nothing
                                (Some("ks/kw"),                      false,   9,   8), // regexp matches 16 (+1 failed)
                                (Some("file"),                       false,   7,   7), // case-insensitive
                                (Some("FILE"),                       false,   7,   7), // case-insensitive
                                (Some("file-next"),                  true,    0,   0), // case-sensitive
                                (Some("File-Next"),                  true,    1,   1), // case-sensitive
        ] {
            parse_hist("test/emerge.10000.log", 1517609348, 1520891098,
                       None, None, true, false, f, e,
                       vec![("start",c1),("stop",c2)]);
        }
    }

    #[test]
    /// Filtering by timestamp
    fn parse_hist_filter_ts() {
        let (umin,umax,fmin,fmax) = (std::i64::MIN, std::i64::MAX, 1517609348, 1520891098);
        for (min,max,c1,c2) in vec![(None,             None,           889, 832),
                                    (Some(umin),       None,           889, 832),
                                    (Some(fmin),       None,           889, 832),
                                    (None,             Some(umax),     889, 832),
                                    (None,             Some(fmax),     889, 832),
                                    (Some(fmin),       Some(fmax),     889, 832),
                                    (Some(fmax),       None,             0,   0),
                                    (None,             Some(fmin),       0,   1), //fist line of this file happens to be a stop
                                    (None,             Some(umin),       0,   0),
                                    (Some(umax),       None,             0,   0),
                                    (Some(1517917751), Some(1517931835), 6,   6),
                                    (Some(1517959010), Some(1518176159), 24, 21),
        ] {
            parse_hist("test/emerge.10000.log", 1517609348, 1520891098,
                       min, max, true, false, None, true,
                       vec![("start",c1),("stop",c2)]);
        }
    }

    #[test]
    /// Enabling and disabling sync
    fn parse_hist_sync_merge() {
        for (m,s,c1,c2,c3,c4) in vec![(true,   true, 889, 832, 163, 150),
                                      (false,  true,   0,   0, 163, 150),
                                      (true,  false, 889, 832,   0,   0),
                                      (false, false,   0,   0,   0,   0),
        ] {
            parse_hist("test/emerge.10000.log", 1517609348, 1520891098,
                       None, None, m, s, None, false,
                       vec![("start",c1),("stop",c2),("syncstart",c3),("syncstop",c4)]);
        }
    }

    fn parse_pretend(filename: &str, expect: &Vec<(&str, &str)>) {
        // Setup
        let pretend = new_pretend(File::open(filename).unwrap(), filename);
        let mut count = 0;
        // Check that all items look valid
        for ParsedPretend{ebuild, version} in pretend {
            assert_eq!(ebuild, expect[count].0);
            assert_eq!(version, expect[count].1);
            count += 1;
        }
    }

    #[test]
    fn parse_pretend_basic() {
        let out = vec![("sys-devel/gcc","6.4.0-r1"),
                       ("sys-libs/readline","7.0_p3"),
                       ("app-portage/emlop","0.1.0_p20180221"),
                       ("app-shells/bash","4.4_p12"),
                       ("dev-db/postgresql","10.3")];
        parse_pretend("test/emerge-p.basic.out", &out);
        parse_pretend("test/emerge-pv.basic.out", &out);
    }

    #[test]
    fn parse_pretend_blocker() {
        let out = vec![("app-admin/syslog-ng","3.13.2"),
                       ("dev-lang/php","7.1.13")];
        parse_pretend("test/emerge-p.blocker.out", &out);
    }

    #[test]
    fn split_atom_() {
        assert_eq!(None, split_atom(""));
        assert_eq!(None, split_atom("a"));
        assert_eq!(None, split_atom("-"));
        assert_eq!(None, split_atom("42"));
        assert_eq!(None, split_atom("-42"));
        assert_eq!(None, split_atom("42-"));
        assert_eq!(None, split_atom("a-/"));
        assert_eq!(Some(("a","0")), split_atom("a-0"));
        assert_eq!(Some(("a","1")), split_atom("a-1"));
        assert_eq!(Some(("a","2")), split_atom("a-2"));
        assert_eq!(Some(("a","3")), split_atom("a-3"));
        assert_eq!(Some(("a","4")), split_atom("a-4"));
        assert_eq!(Some(("a","5")), split_atom("a-5"));
        assert_eq!(Some(("a","6")), split_atom("a-6"));
        assert_eq!(Some(("a","7")), split_atom("a-7"));
        assert_eq!(Some(("a","8")), split_atom("a-8"));
        assert_eq!(Some(("a","9")), split_atom("a-9"));
        assert_eq!(None, split_atom("a-:"));
        assert_eq!(Some(("a-b","2")), split_atom("a-b-2"));
        assert_eq!(Some(("a-b","2-3")), split_atom("a-b-2-3"));
        assert_eq!(Some(("a-b","2-3_r1")), split_atom("a-b-2-3_r1"));
        assert_eq!(Some(("a-b","2foo-4")), split_atom("a-b-2foo-4"));
        assert_eq!(Some(("a-b","2foo-4-")), split_atom("a-b-2foo-4-"));
        assert_eq!(Some(("Noël","2-bêta")), split_atom("Noël-2-bêta"));
    }
}
