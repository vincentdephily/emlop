//! Handles parsing of current emerge state.

use super::Ansi;
use crate::ResumeKind;
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
pub fn get_pretend<R: Read>(reader: R, filename: &str) -> Vec<Pkg> {
    debug!("get_pretend input={}", filename);
    let mut out = vec![];
    let re = Regex::new("^\\[ebuild[^]]*\\] +([^ :\\n]+)").unwrap();
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

#[derive(Deserialize)]
struct Resume {
    mergelist: Vec<Vec<String>>,
}
#[derive(Deserialize)]
struct Mtimedb {
    resume: Option<Resume>,
    resume_backup: Option<Resume>,
}

/// Parse resume list from portage mtimedb
pub fn get_resume(kind: ResumeKind) -> Vec<Pkg> {
    get_resume_priv(kind, "/var/cache/edb/mtimedb").unwrap_or_default()
}
fn get_resume_priv(kind: ResumeKind, file: &str) -> Option<Vec<Pkg>> {
    if kind == ResumeKind::No {
        return Some(vec![]);
    }
    let reader = File::open(file).map_err(|e| warn!("Cannot open {file:?}: {e}")).ok()?;
    let db: Mtimedb = from_reader(reader).map_err(|e| warn!("Cannot parse {file:?}: {e}")).ok()?;
    let r = if kind == ResumeKind::Backup { db.resume_backup? } else { db.resume? };
    Some(r.mergelist.iter().filter_map(|v| v.get(2).and_then(|s| Pkg::try_new(s))).collect())
}


/// Retrieve summary info from the build log
pub fn get_buildlog(pkg: &Pkg, portdir: &str) -> Option<String> {
    let name = format!("{}/portage/{}/temp/build.log", portdir, pkg.ebuild_version());
    info!("Build log: {name}");
    let file = File::open(&name).map_err(|e| warn!("Cannot open {name:?}: {e}")).ok()?;
    Some(read_buildlog(file, 50))
}
fn read_buildlog(file: File, max: usize) -> String {
    let mut last = String::new();
    for line in rev_lines::RevLines::new(BufReader::new(file)).map_while(Result::ok) {
        if line.starts_with(">>>") {
            let tag = line.split_ascii_whitespace().skip(1).take(2).collect::<Vec<_>>().join(" ");
            if last.is_empty() {
                return format!(" ({})", tag.trim_matches('.'));
            } else {
                return format!(" ({}: {})", tag.trim_matches('.'), last);
            }
        }
        if last.is_empty() {
            let stripped = Ansi::strip(&line, max);
            if stripped.chars().any(char::is_alphanumeric) {
                last = stripped;
            }
        }
    }
    format!(" ({last})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    /// Check that `get_pretend()` has the expected output
    fn check_pretend(file: &str, expect: &[(&str, &str)]) {
        let mut n = 0;
        for p in get_pretend(File::open(&format!("tests/{file}")).unwrap(), file) {
            assert_eq!((p.ebuild(), p.version()), expect[n], "Mismatch for {file}:{n}");
            n += 1;
        }
    }

    #[test]
    fn pretend_basic() {
        let out = [("sys-devel/gcc", "6.4.0-r1"),
                   ("sys-libs/readline", "7.0_p3"),
                   ("app-portage/emlop", "0.1.0_p20180221"),
                   ("app-shells/bash", "4.4_p12"),
                   ("dev-db/postgresql", "10.3")];
        check_pretend("emerge-p.basic.out", &out);
        check_pretend("emerge-pv.basic.out", &out);
    }

    #[test]
    fn pretend_blocker() {
        let out = [("app-admin/syslog-ng", "3.13.2"), ("dev-lang/php", "7.1.13")];
        check_pretend("emerge-p.blocker.out", &out);
    }

    /// Check that `get_resume()` has the expected output
    fn check_resume(kind: ResumeKind, file: &str, expect: Option<&[(&str, &str)]>) {
        let file = &format!("tests/{file}");
        match expect {
            Some(ex) => {
                let mut n = 0;
                for p in get_resume_priv(kind, file).unwrap() {
                    assert_eq!((p.ebuild(), p.version()), ex[n], "Mismatch for {file}:{n}");
                    n += 1;
                }
            },
            None => assert_eq!(None, get_resume_priv(kind, file)),
        }
    }

    #[test]
    fn resume() {
        check_resume(ResumeKind::Main,
                     "mtimedb.ok",
                     Some(&[("dev-lang/rust", "1.65.0"), ("app-portage/emlop", "0.5.0")]));
        check_resume(ResumeKind::Backup,
                     "mtimedb.ok",
                     Some(&[("app-portage/dummybuild", "0.1.600"),
                            ("app-portage/dummybuild", "0.1.60")]));
        check_resume(ResumeKind::Main, "mtimedb.empty", Some(&[]));
        check_resume(ResumeKind::Main, "mtimedb.noresume", None);
        check_resume(ResumeKind::Main, "mtimedb.badjson", None);
    }

    #[test]
    fn pkg_new() {
        assert_eq!(Some(Pkg::new("foo", "1.2")), Pkg::try_new("foo-1.2"));
        assert_eq!("foo", Pkg::new("foo", "1.2").ebuild());
        assert_eq!("1.2", Pkg::new("foo", "1.2").version());
        assert_eq!("foo-1.2", Pkg::new("foo", "1.2").ebuild_version());
    }

    #[test]
    fn buildlog() {
        for (file, lim, res) in
            [("build.log.empty", 20, ""),
             ("build.log.notag", 50, "* Upstream:   phil@riverbankcomputing.com pyqt@riv..."),
             ("build.log.onlytag", 30, "Unpacking source"),
             ("build.log.trim", 20, "Unpacking source: 102 |         HTTP2W..."),
             ("build.log.short", 20, "Configuring source: done"),
             ("build.log.color", 100, "Unpacking source: 0:57.55    Compiling syn v1.0.99"),
             ("build.log.color", 15, "Unpacking source: 0:57.55    Comp...")]
        {
            let f = File::open(&format!("tests/{file}")).expect(&format!("can't open {file:?}"));
            assert_eq!(format!(" ({res})"), read_buildlog(f, lim));
        }
    }
}
