//! Handles the basic log parsing.

use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines, stdin, Stdin};

/// Represents one basic event parsed from the file.
pub enum HistEvent {
    /// Emerge started (might never complete)
    Start{ts: i64, ebuild: String, version: String, iter: String},
    /// Emerge completed
    Stop{ts: i64, ebuild: String, version: String, iter: String},
}
pub struct PretendEvent {
    pub ebuild: String,
    pub version: String,
}

pub struct HistParser {
    lines: Lines<BufReader<File>>,
    re_pkg: Option<Regex>,
    re_start: Regex,
    re_stop: Regex,
}
pub struct PretendParser {
    lines: Lines<BufReader<Stdin>>,
    re: Regex,
}

/// Iterates over an emerge log file to return matching `Event`s.
impl HistParser {
    pub fn new(filename: &str, filter: Option<&str>) -> HistParser {
        let file = File::open(filename).unwrap();
        HistParser{lines: BufReader::new(file).lines(),
                   re_pkg: filter.and_then(|pkg| Some(Regex::new(pkg).unwrap())),
                   re_start: Regex::new("^([0-9]+): *>>> emerge \\(([0-9]+ of [0-9]+)\\) (.+?)-([0-9.r-]+) ").unwrap(),
                   re_stop: Regex::new("^([0-9]+): *::: completed emerge \\(([0-9]+ of [0-9]+)\\) (.+?)-([0-9.r-]+) ").unwrap(),
        }
    }
}
/// Iterates over an emerge-pretend output to return matching `Event`s.
impl PretendParser {
    pub fn new() -> PretendParser {
        PretendParser{lines: BufReader::new(stdin()).lines(),
                      re: Regex::new("^\\[[^]]+\\] (.+?)-([0-9.r-]+):").unwrap(),
        }
    }
}

impl Iterator for HistParser {
    type Item = HistEvent;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.lines.next() {
                Some(Ok(ref line)) => {
                    // Try to match this line, loop with the next line if not
                    if let Some(c) = self.re_start.captures(line) {
                        let eb = c.get(3).unwrap().as_str();
                        if self.re_pkg.as_ref().map_or(true, |r| r.is_match(eb)) {
                            return Some(HistEvent::Start{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                                                         ebuild: eb.to_string(),
                                                         iter: c.get(2).unwrap().as_str().to_string(),
                                                         version: c.get(4).unwrap().as_str().to_string()})
                        }
                    };
                    if let Some(c) = self.re_stop.captures(line) {
                        let eb = c.get(3).unwrap().as_str();
                        if self.re_pkg.as_ref().map_or(true, |r| r.is_match(eb)) {
                            return Some(HistEvent::Stop{ts: c.get(1).unwrap().as_str().parse::<i64>().unwrap(),
                                                        ebuild: eb.to_string(),
                                                        iter: c.get(2).unwrap().as_str().to_string(),
                                                        version: c.get(4).unwrap().as_str().to_string()})
                        }
                    };
                },
                _ =>
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
