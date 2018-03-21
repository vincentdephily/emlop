//! Handles the actual log parsing.
//!
//! Instantiate a `HistParser` or `PretendParser` and iterate over it to retrieve the events.

use regex::{Regex, RegexBuilder};
use std::io::{BufRead, BufReader, Lines, Read};

/// Create a closure that matches package depending on options.
fn filter_fn(package: Option<&str>, exact: bool) -> Box<Fn(&str) -> bool> {
    match package {
        // No filter
        None => {
            Box::new(|_| true)
        },
        Some(search) => match exact {
            true => match search.contains("/") {
                // Filter on exact name
                true => {
                    let srch = search.to_string();
                    Box::new(move |pkg| pkg == srch)
                },
                // Filter on exact category/name
                false => {
                    let srch = format!("/{}",search);
                    Box::new(move |pkg| pkg.ends_with(&srch))
                }
            },
            // Filter on case-insensitive regexp
            false => {
                let re = RegexBuilder::new(search)
                    .case_insensitive(true)
                    .build().unwrap();
                Box::new(move |pkg| re.is_match(pkg))
            }
        }
    }
}

/// Items returned from Parser.next().
#[derive(Debug)]
pub enum Parsed {
    /// Emerge started (might never complete)
    Start{ts: i64, ebuild: String, version: String, iter: String, line: String},
    /// Emerge completed
    Stop{ts: i64, ebuild: String, version: String, iter: String, line: String},
    /// Pretend output
    Pretend{ebuild: String, version: String, line: String},
}

/// Iterates over input line by line over the input to produce `Parsed` items.
pub struct Parser<R: Read> {
    input: String,
    lines: Lines<BufReader<R>>,
    curline: u64,
    filter: Box<Fn(&str) -> bool>,
    re_start: Option<Regex>,
    re_stop: Option<Regex>,
    re_pretend: Option<Regex>,
}

impl<R: Read> Parser<R> {
    fn parse_start(&mut self, line: &str) -> Option<Parsed> {
        let re = self.re_start.as_ref()?;
        if !line.contains("> emerge") {return None}
        let c = re.captures(line)?;
        let eb = c.get(3).unwrap().as_str();
        if !(self.filter)(eb) {return None}
        Some(Parsed::Start{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                           ebuild: eb.to_string(),
                           iter: c.get(2).unwrap().as_str().to_string(),
                           version: c.get(4).unwrap().as_str().to_string(),
                           line: line.to_string()})
    }
    fn parse_stop(&mut self, line: &str) -> Option<Parsed> {
        let re = self.re_stop.as_ref()?;
        if !line.contains(": completed") {return None}
        let c = re.captures(line)?;
        let eb = c.get(3).unwrap().as_str();
        if !(self.filter)(eb) {return None}
        Some(Parsed::Stop{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                          ebuild: eb.to_string(),
                          iter: c.get(2).unwrap().as_str().to_string(),
                          version: c.get(4).unwrap().as_str().to_string(),
                          line: line.to_string()})
    }
    fn parse_pretend(&mut self, line: &str) -> Option<Parsed> {
        let re = self.re_pretend.as_ref()?;
        let c = re.captures(line)?;
        Some(Parsed::Pretend{ebuild: c.get(1).unwrap().as_str().to_string(),
                             version: c.get(2).unwrap().as_str().to_string(),
                             line: line.to_string()})
    }

    pub fn new_hist(reader: R, reader_name: &str, search_str: Option<&str>, search_exact: bool) -> Parser<R> {
        Parser{input: reader_name.to_string(),
               lines: BufReader::new(reader).lines(),
               curline: 0,
               filter: filter_fn(search_str, search_exact),
               re_start: Some(Regex::new("^([0-9]+):  >>> emerge \\(([1-9][0-9]* of [1-9][0-9]*)\\) (.+)-([0-9][0-9a-z._-]*) ").unwrap()),
               re_stop: Some(Regex::new("^([0-9]+):  ::: completed emerge \\(([1-9][0-9]* of [1-9][0-9]*)\\) (.+)-([0-9][0-9a-z._-]*) ").unwrap()),
               re_pretend: None,
        }
    }
    pub fn new_pretend(reader: R, reader_name: &str) -> Parser<R> {
        Parser{input: reader_name.to_string(),
               lines: BufReader::new(reader).lines(),
               curline: 0,
               filter: filter_fn(None, true),
               re_start: None,
               re_stop: None,
               re_pretend: Some(Regex::new("^\\[ebuild[^]]+\\] (.+?)-([0-9][0-9a-z._-]*)").unwrap()),
        }
    }
}

impl<R: Read> Iterator for Parser<R> {
    type Item = Parsed;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.curline += 1;
            match self.lines.next() {
                Some(Ok(ref line)) => { // Got a line, see if one of the funs match it
                    if let Some(found) = self.parse_start(line) {return Some(found)}
                    if let Some(found) = self.parse_stop(line) {return Some(found)}
                    if let Some(found) = self.parse_pretend(line) {return Some(found)}
                },
                Some(Err(e)) => // Could be invalid UTF8, system read error...
                    println!("WARN {}:{}: {}", self.input, self.curline, e), // FIXME proper log levels
                None => // End of file
                    return None,
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use ::*;
    use parser::*;
    use std::fs::File;

    /// This checks parsing the given emerge.log.
    fn parse_hist(filename: &str, filter: Option<&str>, exact: bool, mindate: i64, maxdate: i64, expect_count: usize) {
        // Setup
        let hist = Parser::new_hist(File::open(filename).unwrap(), filename, filter, exact);
        let re_atom = Regex::new("^[a-z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap(); //FIXME use catname.txt
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap(); //Should match pattern used in *Parser
        let re_iter = Regex::new("^[1-9][0-9]* of [1-9][0-9]*$").unwrap(); //Should match pattern used in *Parser
        let mut count = 0;
        // Check that all items look valid
        for p in hist {
            count += 1;
            let (ts, ebuild, version, iter, line) = match p {
                Parsed::Start{ts, ebuild, version, iter, line} => (ts, ebuild, version, iter, line),
                Parsed::Stop{ts, ebuild, version, iter, line} => (ts, ebuild, version, iter, line),
                e => {assert!(false, "unexpected {:?}", e);(0,String::from(""),String::from(""),String::from(""),String::from(""))},
            };
            assert!(ts >= mindate && ts <= maxdate, "Out of bound date {} in {}", fmt_time(ts), line);
            assert!(re_atom.is_match(&ebuild), "Invalid ebuild atom {} in {}", ebuild, line);
            assert!(re_version.is_match(&version), "Invalid version {} in {}", version, line);
            assert!(re_iter.is_match(&iter), "Invalid iteration {} in {}", iter, line);
        }
        assert_eq!(count, expect_count, "Got {} events, expected {:?} {:?} {}", count, expect_count, filter, exact);
    }

    #[test]
    /// Simplified emerge log containing all the ebuilds in all the versions of the current portage tree (see test/generate.sh)
    fn parse_hist_all() {
        parse_hist("test/emerge.all.log", None, false,
                   1483228800, 1483747200, // Generated dates are from 2017-01-01 to 2017-01-07
                   74830);    // wc -l < test/emerge.all.log
    }

    #[test]
    /// Emerge log with some null bytes in the middle
    fn parse_hist_nullbytes() {
        parse_hist("test/emerge.nullbytes.log", None, false,
                   1327867709, 1327871057, // Taken from the file
                   28);          // 14 merges
    }

    #[test]
    /// Filtering by package
    fn parse_hist_filter() {
        for (f,e,c) in vec![(None,                               false, 1721), // Everything
                            (Some("kactivities"),                false,    8), // regexp matches 4
                            (Some("kactivities"),                true,     4), // string matches 2
                            (Some("kde-frameworks/kactivities"), true,     4), // string matches 2
                            (Some("frameworks/kactivities"),     true,     0), // string matches nothing
                            (Some("ks/kw"),                      false,   17), // regexp matches 16 (+1 failed)
                            (Some("file"),                       false,   14), // case-insensitive
                            (Some("FILE"),                       false,   14), // case-insensitive
                            (Some("file-next"),                  true,     0), // case-sensitive
                            (Some("File-Next"),                  true,     2), // case-sensitive
        ] {
            parse_hist("test/emerge.10000.log", f, e,
                       1517609348, 1520891098,
                       c);
        }
    }

    fn parse_pretend(filename: &str, expect: &Vec<(&str, &str)>) {
        // Setup
        let pretend = Parser::new_pretend(File::open(filename).unwrap(), filename);
        let mut count = 0;
        // Check that all items look valid
        for p in pretend {
            match p {
                Parsed::Pretend{ebuild, version, line} => {
                    assert_eq!(ebuild, expect[count].0, "{}", line);
                    assert_eq!(version, expect[count].1, "{}", line);
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
}
