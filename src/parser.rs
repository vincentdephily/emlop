//! Handles the basic log parsing.

//use std::convert::From;
use regex::Regex;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;

/// Represents one basic event parsed from the file.
#[derive(Debug)]
pub enum Event {
    /// Emerge started (might never complete)
    Start{ts: i64, ebuild: String, version: String, iter: String},
    /// Emerge completed
    Stop{ts: i64, ebuild: String, version: String, iter: String},
}

pub struct Parser {
    iter: io::Lines<BufReader<File>>,
    re_pkg: Option<Regex>,
    re_start: Regex,
    re_stop: Regex,
}

/// Iterates over an emerge log file to return matching `Event`s.
impl Parser {
    pub fn new(filename: &str, filter: Option<&str>) -> Parser {
        // >>> emerge (1 of 7) sys-apps/paxctl-0.7-r2 to /
        // ::: completed emerge (2 of 7) net-libs/liblockfile-1.09 to /
        let file = File::open(filename).unwrap();
        Parser{iter: io::BufReader::new(file).lines(),
               re_pkg: match filter {Some(pkg) => Some(Regex::new(pkg).unwrap()), None => None},
               re_start: Regex::new("^([0-9]+): *>>> emerge \\(([0-9]+ of [0-9]+)\\) (.+?)-([0-9.r-]+) ").unwrap(),
               re_stop:  Regex::new("^([0-9]+): *::: completed emerge \\(([0-9]+ of [0-9]+)\\) (.+?)-([0-9.r-]+) ").unwrap(),
        }
    }
    fn is_pkg_match(&self, s: &str) -> bool {
        match self.re_pkg {
            None => true,
            Some(ref r) => r.is_match(s),
        }
    }
}

impl Iterator for Parser {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Ok(ref line)) => {
                    // Check if this is a line we're interested in.
                    if let Some(c) = self.re_start.captures(line) {
                        let eb = c.get(3).unwrap().as_str();
                        if self.is_pkg_match(eb) {
                            return Some(Event::Start{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                                                     ebuild: eb.to_string(),
                                                     iter: c.get(2).unwrap().as_str().to_string(),
                                                     version: c.get(4).unwrap().as_str().to_string()})
                        }
                    }
                    if let Some(c) = self.re_stop.captures(line) {
                        let eb = c.get(3).unwrap().as_str();
                        if self.is_pkg_match(eb) {
                            return Some(Event::Stop{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                                                    ebuild: eb.to_string(),
                                                    iter: c.get(2).unwrap().as_str().to_string(),
                                                    version: c.get(4).unwrap().as_str().to_string()})
                        }
                    }
                    // This line wasn't what we wanted, loop.
                },
                _ =>
                    // End of file
                    return None,
            }
        }
    }
}
