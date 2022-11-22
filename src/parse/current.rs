//! Handles parsing of current emerge state.
//!
//! Use `new_pretend()` to parse and retrieve `Pretend` structs.

use log::*;
use regex::Regex;
use std::io::{BufRead, BufReader, Read};

/// Items sent on the channel returned by `new_pretend()`.
#[derive(Debug)]
pub struct Pretend {
    pub ebuild: String,
    pub version: String,
}

/// Parse portage pretend output into a Vec of `Parsed` enums.
pub fn new_pretend<R: Read>(reader: R, filename: &str) -> Vec<Pretend>
    where R: Send + 'static
{
    debug!("new_pretend input={}", filename);
    let mut out: Vec<Pretend> = vec![];
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

fn parse_pretend(line: &str, re: &Regex) -> Option<Pretend> {
    let c = re.captures(line)?;
    Some(Pretend { ebuild: c.get(1).unwrap().as_str().to_string(),
                   version: c.get(2).unwrap().as_str().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn parse_pretend(filename: &str, expect: &Vec<(&str, &str)>) {
        // Setup
        let pretend = new_pretend(File::open(filename).unwrap(), filename);
        let mut count = 0;
        // Check that all items look valid
        for Pretend { ebuild, version } in pretend {
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
