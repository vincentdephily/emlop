//! Handles the actual log parsing.
//!
//! Instantiate a `Parser` and iterate over it to retrieve the events.

use ::fmt_time;

use crossbeam_channel::{unbounded, Receiver, Sender};
use failure::Error;
use regex::{Regex, RegexBuilder};
use std;
use std::{io::{BufRead, BufReader, Read}, thread};

/// Create a closure that matches timestamp depending on options.
fn filter_ts_fn(min: Option<i64>, max: Option<i64>) -> impl Fn(i64) -> bool {
    // The match patterns are identical but split for readability
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
    let fp = match (&package, exact, package.as_ref().map_or(false, |p| p.contains("/"))) {
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

/// Items returned from Parser.next().
#[derive(Debug)]
pub enum Parsed {
    /// Emerge started (might never complete)
    Start{ts: i64, ebuild: String, version: String, iter: String},
    /// Emerge completed
    Stop{ts: i64, ebuild: String, version: String, iter: String},
    /// Pretend output
    Pretend{ebuild: String, version: String},
}

fn parse_start(line: &str, filter_ts: impl Fn(i64) -> bool, filter_pkg: impl Fn(&str) -> bool) -> Option<Parsed> {
    let (ts_str,rest) = line.split_at(line.find(':')?);
    if !rest.starts_with(":  >>> emerge") {return None}
    let ts = ts_str.parse::<i64>().ok()?;
    if !(filter_ts)(ts) {return None}
    let mut tokens = rest.split_whitespace();
    let (t3,t5,t6) = (tokens.nth(3)?, tokens.nth(1)?, tokens.nth(0)?);
    let (ebuild,version) = split_atom(t6)?;
    if !(filter_pkg)(ebuild) {return None}
    Some(Parsed::Start{ts: ts,
                       ebuild: ebuild.to_string(),
                       iter: format!("{} {}", t3, t5),
                       version: version.to_string()})
}
fn parse_stop(line: &str, filter_ts: impl Fn(i64) -> bool, filter_pkg: impl Fn(&str) -> bool) -> Option<Parsed> {
    let (ts_str,rest) = line.split_at(line.find(':')?);
    if !rest.starts_with(":  ::: completed") {return None}
    let ts = ts_str.parse::<i64>().ok()?;
    if !(filter_ts)(ts) {return None}
    let mut tokens = rest.split_whitespace();
    let (t4,t6,t7) = (tokens.nth(4)?, tokens.nth(1)?, tokens.nth(0)?);
    let (ebuild,version) = split_atom(t7)?;
    if !(filter_pkg)(ebuild) {return None}
    Some(Parsed::Stop{ts: ts,
                      ebuild: ebuild.to_string(),
                      iter: format!("{} {}", t4, t6),
                      version: version.to_string()})
}
fn parse_pretend(line: &str, re: &Regex) -> Option<Parsed> {
    let c = re.captures(line)?;
    Some(Parsed::Pretend{ebuild: c.get(1).unwrap().as_str().to_string(),
                         version: c.get(2).unwrap().as_str().to_string()})
}

/// Parse emerge log into a channel of `Parsed` enums.
pub fn new_hist<R: Read>(reader: R, filename: &str, min_ts: Option<i64>, max_ts: Option<i64>, search_str: Option<&str>, search_exact: bool) -> Result<Receiver<Parsed>, Error> where R: std::marker::Send+'static {
    debug!("new_hist input={} min={:?} max={:?} str={:?} exact={}", filename, min_ts, max_ts, search_str, search_exact);
    let (tx, rx): (Sender<Parsed>, Receiver<Parsed>) = unbounded();
    let filter_ts = filter_ts_fn(min_ts, max_ts);
    let filter_pkg = filter_pkg_fn(search_str, search_exact)?;
    let filename = filename.to_string();
    thread::spawn(move || {
        for (curline,l) in BufReader::new(reader).lines().enumerate() {
            match l {
                Ok(ref line) => { // Got a line, see if one of the funs match it
                    if let Some(found) = parse_start(line, &filter_ts, &filter_pkg) {tx.send(found)}
                    else if let Some(found) = parse_stop(line, &filter_ts, &filter_pkg) {tx.send(found)}
                },
                Err(e) => // Could be invalid UTF8, system read error...
                    warn!("{}:{}: {}", filename, curline, e),
            }
        }
    });
    Ok(rx)
}

/// Parse portage pretend output into a Vec of `Parsed` enums.
pub fn new_pretend<R: Read>(reader: R, filename: &str) -> Vec<Parsed> where R: std::marker::Send+'static {
    debug!("new_pretend input={}", filename);
    let mut out: Vec<Parsed> = vec![];
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


#[cfg(test)]
mod tests {
    use parser::*;
    use std::collections::HashMap;
    use std::fs::File;

    /// This checks parsing the given emerge.log.
    fn parse_hist(filename: &str, mints: i64, maxts: i64,
                  filter_mints: Option<i64>, filter_maxts: Option<i64>,
                  filter_pkg: Option<&str>, exact: bool,
                  mut expect_counts: Vec<(&str, usize)>) {
        // Setup
        let hist = new_hist(File::open(filename).unwrap(), filename.into(), filter_mints, filter_maxts, filter_pkg, exact).unwrap();
        let re_atom = Regex::new("^[a-z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap(); //FIXME use catname.txt
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap(); //Should match pattern used in *Parser
        let re_iter = Regex::new("^\\([1-9][0-9]* [1-9][0-9]*\\)$").unwrap(); //Should match pattern used in *Parser
        let mut counts: HashMap<String, usize> = HashMap::new();
        // Check that all items look valid
        for p in hist {
            let (kind, ts, ebuild, version, iter) = match p {
                Parsed::Start{ts, ebuild, version, iter} => ("start",   ts, ebuild, version, iter),
                Parsed::Stop{ts, ebuild, version, iter} =>  ("stop",    ts, ebuild, version, iter),
                Parsed::Pretend{ebuild, version} =>         ("pretend", 0,  ebuild, version, String::new()),
            };
            *counts.entry(kind.to_string()).or_insert(0) += 1;
            *counts.entry(ebuild.clone()).or_insert(0) += 1;
            assert!(ts >= filter_mints.unwrap_or(mints) && ts <= filter_maxts.unwrap_or(maxts), "Out of bound date {}", fmt_time(ts));
            assert!(re_atom.is_match(&ebuild), "Invalid ebuild atom {}", ebuild);
            assert!(re_version.is_match(&version), "Invalid version {}", version);
            assert!(re_iter.is_match(&iter), "Invalid iteration {}", iter);
        }
        // Check that we got the right number of each kind (always expect 0 pretend)
        expect_counts.push(("pretend",0));
        for (t, ref c) in expect_counts {
            let v = counts.get(t).unwrap_or(&0);
            assert_eq!(v, c, "Got {} {}, expected {:?} with {:?} {} {:?} {:?}", v, t, c, filter_pkg, exact, filter_mints, filter_maxts);
        }
    }

    #[test]
    /// Simplified emerge log containing all the ebuilds in all the versions of the current portage tree (see test/generate.sh)
    fn parse_hist_all() {
        parse_hist("test/emerge.all.log", 1483228800, 1483747200,
                   None, None, None, false, vec![("start",37415),("stop",37415)]);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_nullbytes() {
        parse_hist("test/emerge.nullbytes.log", 1327867709, 1327871057,
                   None, None, None, false, vec![("start",14),("stop",14)]);
    }
    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badtimestamp() {
        parse_hist("test/emerge.badtimestamp.log", 1327867709, 1327871057,
                   None, None, None, false, vec![("start",2),("stop",3),
                                                 ("media-libs/jpeg",1),    //letter in timestamp
                                                 ("dev-libs/libical",2),
                                                 ("media-libs/libpng",2)]);
    }
    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badversion() {
        parse_hist("test/emerge.badversion.log", 1327867709, 1327871057,
                   None, None, None, false, vec![("start",3),("stop",2),
                                                 ("media-libs/jpeg",2),
                                                 ("dev-libs/libical",2),
                                                 ("media-libs/libpng",1)]); //missing version
    }
    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_shortline() {
        parse_hist("test/emerge.shortline.log", 1327867709, 1327871057,
                   None, None, None, false, vec![("start",3),("stop",2),
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
                       None, None, f, e, vec![("start",c1),("stop",c2)]);
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
                       min, max, None, true, vec![("start",c1),("stop",c2)]);
        }
    }

    fn parse_pretend(filename: &str, expect: &Vec<(&str, &str)>) {
        // Setup
        let pretend = new_pretend(File::open(filename).unwrap(), filename);
        let mut count = 0;
        // Check that all items look valid
        for p in pretend {
            match p {
                Parsed::Pretend{ebuild, version} => {
                    assert_eq!(ebuild, expect[count].0);
                    assert_eq!(version, expect[count].1);
                },
                e => assert!(false, "unexpected {:?}", e),
            }
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
