//! Handles parsing of current emerge state.

use anyhow::{Context, Error};
use log::*;
use regex::Regex;
use serde::Deserialize;
use serde_json::from_reader;
use std::{fs::File,
          io::{BufRead, BufReader, Read}};

/// Package name and version
#[derive(Debug, PartialEq, Eq)]
pub struct Pkg {
    pub ebuild: String,
    pub version: String,
}

/// Parse portage pretend output
pub fn get_pretend<R: Read>(reader: R, filename: &str) -> Vec<Pkg>
    where R: Send + 'static
{
    debug!("get_pretend input={}", filename);
    let mut out = vec![];
    let re = Regex::new("^\\[ebuild[^]]+\\] (.+?)-([0-9][0-9a-z._-]*)").unwrap();
    let mut curline = 1;
    let mut buf = BufReader::new(reader);
    let mut line = String::new();
    loop {
        match buf.read_line(&mut line) {
            // End of file
            Ok(0) => break,
            // Got a line, see if one of the funs match it
            Ok(_) => {
                if let Some(found) = parse_pretend(&line, &re) {
                    out.push(found)
                }
            },
            // Could be invalid UTF8, system read error...
            Err(e) => {
                warn!("{}:{}: {}", filename, curline, e)
            },
        }
        line.clear();
        curline += 1;
    }
    out
}

fn parse_pretend(line: &str, re: &Regex) -> Option<Pkg> {
    let c = re.captures(line)?;
    Some(Pkg { ebuild: c.get(1).unwrap().as_str().to_string(),
               version: c.get(2).unwrap().as_str().to_string() })
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
        Some(r) => Ok(r.mergelist.iter().filter_map(|v| v.get(2).and_then(parse_atom)).collect()),
        None => Ok(vec![]),
    }
}

fn parse_atom(atom: &String) -> Option<Pkg> {
    let mut pos = 0;
    loop {
        pos += atom[pos..].find('-')?;
        if pos > 0 && atom.as_bytes().get(pos + 1)?.is_ascii_digit() {
            return Some(Pkg { ebuild: String::from(&atom[..pos]),
                              version: String::from(&atom[pos + 1..]) });
        }
        pos += 1;
    }
}

/// Retrieve summary info from the build log
pub fn get_buildlog(ebuild: &str, version: &str) -> Option<String> {
    let file = format!("/var/tmp/portage/{}-{}/temp/build.log", ebuild, version);
    let reader = File::open(&file).map_err(|e| warn!("Cannot open {:?}: {e}", file)).ok()?;
    let mut last = None;
    for line in rev_lines::RevLines::new(BufReader::new(reader)).ok()? {
        if last.is_none() {
            last = Some(line.chars().take(30).collect::<String>());
        }
        if line.starts_with(">>>") {
            let tag = line.split_whitespace().skip(1).take(2).collect::<Vec<&str>>().join(" ");
            return Some(format!("  ({}: {}...)", tag.trim_matches('.'), last?.trim()))
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
        for Pkg { ebuild, version } in pretend {
            assert_eq!(ebuild, expect[count].0);
            assert_eq!(version, expect[count].1);
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
}
