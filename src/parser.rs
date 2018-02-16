//! Handles the actual log parsing.
//!
//! Instantiate a `HistParser` or `PretendParser` and iterate over it to retrieve the events.

use regex::Regex;
use std::fs::File;
use std::io::{stdin, BufRead, BufReader, Lines, Stdin};

/// Represents one emerge event parsed from an emerge.log file.
pub enum HistEvent {
    /// Emerge started (might never complete)
    Start{ts: i64, ebuild: String, version: String, iter: String, line: String},
    /// Emerge completed
    Stop{ts: i64, ebuild: String, version: String, iter: String, line: String},
}
/// Represents one emerge-pretend parsed from an `emerge -p` output.
pub struct PretendEvent {
    pub ebuild: String,
    pub version: String,
}

/// Iterates over an emerge log file to return matching `Event`s.
pub struct HistParser {
    filename: String,
    lines: Lines<BufReader<File>>,
    curline: u64,
    re_pkg: Option<Regex>,
    re_start: Regex,
    re_stop: Regex,
}
/// Iterates over an emerge-pretend output to return matching `Event`s.
pub struct PretendParser {
    lines: Lines<BufReader<Stdin>>,
    re: Regex,
}

impl HistParser {
    pub fn new(filename: &str, filter: Option<&str>) -> HistParser {
        let file = File::open(filename).unwrap();
        HistParser{filename: filename.to_string(),
                   lines: BufReader::new(file).lines(),
                   curline: 0,
                   re_pkg: filter.and_then(|pkg| Some(Regex::new(pkg).unwrap())),
                   re_start: Regex::new("^([0-9]+): *>>> emerge \\(([1-9][0-9]* of [1-9][0-9]*)\\) (.+?)-([0-9][0-9a-z._-]*) ").unwrap(),
                   re_stop: Regex::new("^([0-9]+): *::: completed emerge \\(([1-9][0-9]* of [1-9][0-9]*)\\) (.+?)-([0-9][0-9a-z._-]*) ").unwrap(),
        }
    }
}
impl PretendParser {
    pub fn new() -> PretendParser {
        PretendParser{lines: BufReader::new(stdin()).lines(),
                      re: Regex::new("^\\[[^]]+\\] (.+?)-([0-9.r-]+)(:| |$)").unwrap(),
        }
    }
}

impl Iterator for HistParser {
    type Item = HistEvent;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.lines.next() {
                Some(Ok(ref line)) => {
                    self.curline += 1;
                    // Try to match this line, loop with the next line if not
                    // We do a quick string search before attempting the comparatively slow regexp parsing
                    if line.contains("> emerge") {
                        if let Some(c) = self.re_start.captures(line) {
                            let eb = c.get(3).unwrap().as_str();
                            if self.re_pkg.as_ref().map_or(true, |r| r.is_match(eb)) {
                                return Some(HistEvent::Start{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                                                             ebuild: eb.to_string(),
                                                             iter: c.get(2).unwrap().as_str().to_string(),
                                                             version: c.get(4).unwrap().as_str().to_string(),
                                                             line: line.to_string()})
                            }
                        }
                    }
                    if line.contains(": completed") {
                        if let Some(c) = self.re_stop.captures(line) {
                            let eb = c.get(3).unwrap().as_str();
                            if self.re_pkg.as_ref().map_or(true, |r| r.is_match(eb)) {
                                return Some(HistEvent::Stop{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                                                            ebuild: eb.to_string(),
                                                            iter: c.get(2).unwrap().as_str().to_string(),
                                                            version: c.get(4).unwrap().as_str().to_string(),
                                                            line: line.to_string()})
                            }
                        }
                    }
                },
                Some(Err(e)) => {
                    // Could be invalid UTF8, system read error...
                    self.curline += 1;
                    println!("WARN {}:{}: {:?}", self.filename, self.curline, e) // FIXME proper log levels
                },
                None =>
                    // End of file
                    return None,
            }
        }
    }
}
impl Iterator for PretendParser {
    type Item = PretendEvent;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.lines.next() {
                Some(Ok(ref line)) => {
                    // Try to match this line, loop with the next line if not
                    if let Some(c) = self.re.captures(line) {
                        return Some(PretendEvent{ebuild: c.get(1).unwrap().as_str().to_string(),
                                                 version: c.get(2).unwrap().as_str().to_string()})
                    };
                },
                _ =>
                    // End of file
                    return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ::*;
    use parser::*;

    /// This checks parsing the given emerge.log.
    fn parse_hist(filename: &str, mindate: i64, maxdate: i64, mincount: usize, maxcount: Option<usize>) {
        // Setup
        let hist = HistParser::new(filename, None);
        let re_atom = Regex::new("^[a-z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap(); //FIXME use catname.txt
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap(); //Should match pattern used in *Parser
        let re_iter = Regex::new("^[1-9][0-9]* of [1-9][0-9]*$").unwrap(); //Should match pattern used in *Parser
        let mut count = 0;
        // Check that all items look valid
        for item in hist {
            count += 1;
            let (ts, ebuild, version, iter, line) = match item {
                HistEvent::Start{ts, ebuild, version, iter, line} => (ts, ebuild, version, iter, line),
                HistEvent::Stop{ts, ebuild, version, iter, line} => (ts, ebuild, version, iter, line),
            };
            assert!(ts >= mindate && ts <= maxdate, "Out of bound date {} in {}", fmt_time(ts), line);
            assert!(re_atom.is_match(&ebuild), "Invalid ebuild atom {} in {}", ebuild, line);
            assert!(re_version.is_match(&version), "Invalid version {} in {}", version, line);
            assert!(re_iter.is_match(&iter), "Invalid iteration {} in {}", iter, line);
        }
        assert!(count >= mincount && count <= maxcount.unwrap_or(count), "Got {} events, expected {:?}", count, (mincount, maxcount));
    }

    #[test]
    /// Simplified emerge log containing all the ebuilds in all the versions of the current portage tree (see test/generate.sh)
    fn parse_hist_all() {
        parse_hist("test/emerge.all.log",
                   1483228800, 1483747200, // Generated dates are from 2017-01-01 to 2017-01-07
                   74718, Some(74718));    // wc -l < test/emerge.all.log
    }

    #[test]
    /// Emerge log with some null bytes in the middle
    fn parse_hist_nullbytes() {
        parse_hist("test/emerge.nullbytes.log",
                   1327867709, 1327871057, // Taken from the file
                   28, Some(28));          // 14 merges
    }

    #[test]
    /// Local emerge log
    fn parse_hist_local() {
        parse_hist("/var/log/emerge.log",
                   946684800, epoch_now(), // date from 2000-01-01 to now
                   100, None);
    }
}
