//! Handles parsing of current emerge state.

use anyhow::{Context, Error};
use log::*;
use regex::Regex;
use serde::Deserialize;
use serde_json::from_reader;
use std::{fs::File,
          io::{BufRead, BufReader, Read}};

/// Package name and version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pkg {
    key: String,
    pos: usize,
}
impl Pkg {
    pub fn new(ebuild: &str, version: &str) -> Self {
        Self { key: format!("{ebuild}-{version}"), pos: ebuild.len() + 1 }
    }
    // Algorithm is taken from history.rs and more thoroughly tested there
    fn try_new(key: &str) -> Option<Self> {
        let mut pos = 0;
        loop {
            pos += key[pos..].find('-')?;
            if pos > 0 && key.as_bytes().get(pos + 1)?.is_ascii_digit() {
                return Some(Self { key: key.to_string(), pos: pos + 1 });
            }
            pos += 1;
        }
    }
    pub fn ebuild(&self) -> &str {
        &self.key[..(self.pos - 1)]
    }
    #[cfg(test)]
    pub fn version(&self) -> &str {
        &self.key[self.pos..]
    }
    pub fn ebuild_version(&self) -> &str {
        &self.key
    }
}

/// Parse portage pretend output
pub fn get_pretend<R: Read>(reader: R, filename: &str) -> Vec<Pkg>
    where R: Send + 'static
{
    debug!("get_pretend input={}", filename);
    let mut out = vec![];
    let re = Regex::new("^\\[ebuild[^]]+\\] (.+?-[0-9][0-9a-z._-]*)").unwrap();
    let mut buf = BufReader::new(reader);
    let mut line = String::new();
    loop {
        match buf.read_line(&mut line) {
            // End of file or some other error
            Ok(0) | Err(_) => break,
            // Got a line, see if it's a pkg merge
            Ok(_) => {
                if let Some(c) = re.captures(&line) {
                    out.push(Pkg::try_new(c.get(1).unwrap().as_str()).unwrap())
                }
            },
        }
        line.clear();
    }
    out
}

#[derive(Deserialize, Debug)]
struct Resume {
    mergelist: Vec<Vec<String>>,
}
#[derive(Deserialize, Debug)]
struct Mtimedb {
    resume: Option<Resume>,
}

/// Parse portage mtimedb
pub fn get_resume() -> Result<Vec<Pkg>, Error> {
    let file = "/var/cache/edb/mtimedb";
    let reader = File::open(file).with_context(|| format!("Cannot open {:?}", file))?;
    let db: Mtimedb = from_reader(reader).with_context(|| format!("Cannot parse {:?}", file))?;
    match db.resume {
        Some(r) => {
            Ok(r.mergelist.iter().filter_map(|v| v.get(2).and_then(|s| Pkg::try_new(s))).collect())
        },
        None => Ok(vec![]),
    }
}

/// Simple Ansi escape parser, sufficient to strip text styling.
///
/// More exotic escapes (that shouldn't comme up in build.log) will cause the rest of the string to
/// be interpreted as a sequence, and stripped. There are crates implementing full ansi support, but
/// they seem overkill for our needs.
#[derive(PartialEq)]
enum Ansi {
    /// Normal text
    Txt,
    /// Entered escape sequence
    Esc,
    /// Control Sequence Introducer, includes text styleing and cursor control
    EscCSI,
    /// Unimplemented escape type, this variant is a dead-end
    EscUnsupported,
    /// Finished the escape sequence, but not Txt yet
    EscEnd,
}
impl Ansi {
    fn step(&mut self, c: char) {
        use Ansi::*;
        *self = match self {
            Txt | EscEnd if c == '\x1B' => Esc,
            Txt | EscEnd => Txt,
            Esc if c == '[' => EscCSI,
            Esc => EscUnsupported,
            EscCSI if ('@'..='~').contains(&c) => EscEnd,
            EscCSI => EscCSI,
            EscUnsupported => EscUnsupported,
        }
    }
    fn strip(s: &str, max: usize) -> String {
        let mut out = String::with_capacity(max + 3);
        let mut state = Self::Txt;
        for c in s.trim().chars() {
            state.step(c);
            if state == Self::Txt {
                out.push(c);
                if out.len() >= max {
                    out += "...";
                    break;
                }
            }
        }
        out
    }
}

/// Retrieve summary info from the build log
pub fn get_buildlog(pkg: &Pkg) -> Option<String> {
    let file = format!("/var/tmp/portage/{}/temp/build.log", pkg.ebuild_version());
    let reader = File::open(&file).map_err(|e| warn!("Cannot open {:?}: {e}", file)).ok()?;
    let mut last = None;
    for line in rev_lines::RevLines::new(BufReader::new(reader)).ok()? {
        if last.is_none() {
            last = Some(Ansi::strip(&line, 50));
        }
        if line.starts_with(">>>") {
            let tag = line.split_whitespace().skip(1).take(2).collect::<Vec<&str>>().join(" ");
            return Some(format!(" ({}: {})", tag.trim_matches('.'), last?));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn parse_pretend(filename: &str, expect: &Vec<(&str, &str)>) {
        // Setup
        let pretend = get_pretend(File::open(filename).unwrap(), filename);
        let mut count = 0;
        // Check that all items look valid
        for p in pretend {
            assert_eq!(p.ebuild(), expect[count].0);
            assert_eq!(p.version(), expect[count].1);
            count += 1;
        }
    }

    #[test]
    fn parse_pretend_basic() {
        let out = vec![("sys-devel/gcc", "6.4.0-r1"),
                       ("sys-libs/readline", "7.0_p3"),
                       ("app-portage/emlop", "0.1.0_p20180221"),
                       ("app-shells/bash", "4.4_p12"),
                       ("dev-db/postgresql", "10.3")];
        parse_pretend("test/emerge-p.basic.out", &out);
        parse_pretend("test/emerge-pv.basic.out", &out);
    }

    #[test]
    fn parse_pretend_blocker() {
        let out = vec![("app-admin/syslog-ng", "3.13.2"), ("dev-lang/php", "7.1.13")];
        parse_pretend("test/emerge-p.blocker.out", &out);
    }

    #[test]
    fn pkg_new() {
        assert_eq!(Some(Pkg::new("foo", "1.2")), Pkg::try_new("foo-1.2"));
        assert_eq!("foo", Pkg::new("foo", "1.2").ebuild());
        assert_eq!("1.2", Pkg::new("foo", "1.2").version());
        assert_eq!("foo-1.2", Pkg::new("foo", "1.2").ebuild_version());
    }
}
