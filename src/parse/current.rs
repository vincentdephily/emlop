//! Handles parsing of current emerge state.

use super::{Ansi, ProcKind, ProcList};
use crate::ResumeKind;
use libc::pid_t;
use log::*;
use regex::Regex;
use serde::Deserialize;
use serde_json::from_reader;
use std::{collections::HashMap,
          fs::File,
          io::{BufRead, BufReader, Read},
          path::PathBuf,
          time::Instant};

/// Package name and version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pkg {
    key: String,
    pos: usize,
    pub bin: bool,
}
impl Pkg {
    // Algorithm is taken from history.rs and more thoroughly tested there
    pub fn try_new(key: &str, bin: bool) -> Option<Self> {
        let mut pos = 0;
        loop {
            pos += key[pos..].find('-')?;
            if pos > 0 && key.as_bytes().get(pos + 1)?.is_ascii_digit() {
                return Some(Self { key: key.to_string(), pos: pos + 1, bin });
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
    let re = Regex::new("^\\[([a-z]+)[^]]*\\] +([^ :\\n]+)").unwrap();
    let mut buf = BufReader::new(reader);
    let mut line = String::new();
    loop {
        match buf.read_line(&mut line) {
            // End of file or some other error
            Ok(0) | Err(_) => break,
            // Got a line, see if it's a pkg merge
            Ok(_) => {
                if let Some(c) = re.captures(&line) {
                    let bin = match &c[1] {
                        "ebuild" => false,
                        "binary" => true,
                        _ => continue,
                    };
                    match Pkg::try_new(&c[2], bin) {
                        Some(p) => out.push(p),
                        None => warn!("Cannot parse {line}"),
                    }
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
#[derive(Deserialize, Default)]
pub struct Mtimedb {
    resume: Option<Resume>,
    resume_backup: Option<Resume>,
    updates: Option<HashMap<String, i64>>,
}
impl Mtimedb {
    pub fn new() -> Self {
        Self::try_new("/var/cache/edb/mtimedb").unwrap_or_default()
    }
    fn try_new(file: &str) -> Option<Self> {
        let now = Instant::now();
        let reader = File::open(file).map_err(|e| warn!("Cannot open {file:?}: {e}")).ok()?;
        let r = from_reader(reader).map_err(|e| warn!("Cannot parse {file:?}: {e}")).ok();
        debug!("Loaded {file} in {:?}", now.elapsed());
        r
    }
}


/// Parse resume list from portage mtimedb
pub fn get_resume(kind: ResumeKind, db: &Mtimedb) -> Vec<Pkg> {
    let r = try_get_resume(kind, db).unwrap_or_default();
    debug!("Loaded {kind:?} resume list: {r:?}");
    r
}
fn try_get_resume(kind: ResumeKind, db: &Mtimedb) -> Option<Vec<Pkg>> {
    let r = match kind {
        ResumeKind::Either | ResumeKind::Auto => {
            db.resume.as_ref().filter(|o| !o.mergelist.is_empty()).or(db.resume_backup.as_ref())?
        },
        ResumeKind::Main => db.resume.as_ref()?,
        ResumeKind::Backup => db.resume_backup.as_ref()?,
        ResumeKind::No => return Some(vec![]),
    };
    Some(r.mergelist
          .iter()
          .filter_map(|v| {
              v.get(2).and_then(|s| Pkg::try_new(s, v.first().is_some_and(|b| b == "binary")))
          })
          .collect())
}


pub struct PkgMoves(HashMap<String, String>);
impl PkgMoves {
    /// Parse package moves using file list from portagedb
    pub fn new(db: &Mtimedb) -> Self {
        let r = Self::try_new(db).unwrap_or_default();
        trace!("Package moves: {r:?}");
        Self(r)
    }

    pub fn get(&self, key: String) -> String {
        self.0.get(&key).cloned().unwrap_or(key)
    }

    pub fn get_ref<'a>(&'a self, key: &'a String) -> &'a String {
        self.0.get(key).unwrap_or(key)
    }

    /// Compare update file names in reverse chronological order
    fn cmp_update_files(a: &&String, b: &&String) -> std::cmp::Ordering {
        // Find the file part
        let a = a[a.rfind('/').map(|n| n + 1).unwrap_or(0)..].as_bytes();
        let b = b[b.rfind('/').map(|n| n + 1).unwrap_or(0)..].as_bytes();
        // If it looks like "Quarter-Year", rewrite it as "YearQuarter"
        let a = if let &[q, b'Q', b'-', y1, y2, y3, y4] = a { &[y1, y2, y3, y4, q] } else { a };
        let b = if let &[q, b'Q', b'-', y1, y2, y3, y4] = b { &[y1, y2, y3, y4, q] } else { b };
        b.cmp(a)
    }

    /// Load, sort and parse update files
    fn try_new(db: &Mtimedb) -> Option<HashMap<String, String>> {
        let now = Instant::now();
        let mut files: Vec<_> = db.updates.as_ref()?.keys().collect();
        files.sort_unstable_by(Self::cmp_update_files);
        let mut moves = HashMap::new();
        for f in &files {
            Self::parse(&mut moves, f);
        }
        debug!("Loaded {} package moves from {} files in {:?}",
               moves.len(),
               files.len(),
               now.elapsed());
        Some(moves)
    }

    fn parse(moves: &mut HashMap<String, String>, file: &str) -> Option<()> {
        trace!("Parsing {file}");
        let f = File::open(file).map_err(|e| warn!("Cannot open {file:?}: {e}")).ok()?;
        for line in
            BufReader::new(f).lines().map_while(Result::ok).filter(|l| l.starts_with("move "))
        {
            if let Some((from, to)) = line[5..].split_once(' ') {
                // Portage rewrites each repo's update files so that entries point directly to the
                // final name, but there can still be cross-repo chains, which we untangle
                // here. Assumes we're parsing files newest-first.
                if let Some(to_final) = moves.get(to) {
                    if from != to_final {
                        trace!("Using move {from} -> {to_final} instead -> {to} in {file}");
                        moves.insert(from.to_owned(), to_final.clone());
                    } else {
                        trace!("Ignoring move {from} -> {to} in {file}");
                    }
                } else {
                    // TODO: MSRV 1.?? try_insert https://github.com/rust-lang/rust/issues/82766
                    moves.entry(from.to_owned()).or_insert_with(|| to.to_owned());
                }
            }
        }
        Some(())
    }
}


/// Retrieve summary info from the build log
pub fn get_buildlog(pkg: &Pkg, portdirs: &Vec<PathBuf>) -> Option<String> {
    for portdir in portdirs {
        let name = portdir.join("portage").join(pkg.ebuild_version()).join("temp/build.log");
        if let Ok(file) = File::open(&name).map_err(|e| warn!("Cannot open {name:?}: {e}")) {
            info!("Build log: {}", name.display());
            return Some(read_buildlog(file, 50));
        }
    }
    None
}
fn read_buildlog(file: File, max: usize) -> String {
    let mut last = String::new();
    for line in rev_lines::RevLines::new(BufReader::new(file)).map_while(Result::ok) {
        if line.starts_with(">>>") {
            let tag = line.split_ascii_whitespace().skip(1).take(2).collect::<Vec<_>>().join(" ");
            return if last.is_empty() {
                format!(" ({})", tag.trim_matches('.'))
            } else {
                format!(" ({}: {})", tag.trim_matches('.'), last)
            };
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

#[derive(Debug)]
pub struct EmergeInfo {
    pub start: i64,
    pub roots: Vec<pid_t>,
    pub pkgs: Vec<Pkg>,
}

/// Get info from currently running emerge processes
///
/// * emerge /usr/lib/python-exec/python3.11/emerge -Ov1 dummybuild
///   gives us the emerge command, and the tmpdir (looking at open fds)
/// * python3.11 /usr/lib/portage/python3.11/pid-ns-init 250 250 250 18 0,1,2 /usr/bin/sandbox
///   [app-portage/dummybuild-0.1.600] sandbox /usr/lib/portage/python3.11/ebuild.sh unpack
///   gives us the actually emerging ebuild and stage (depends on portage FEATURES=sandbox, which
///   should be the case for almost all users)
pub fn get_emerge(procs: &ProcList) -> EmergeInfo {
    let mut res = EmergeInfo { start: i64::MAX, roots: vec![], pkgs: vec![] };
    for (pid, proc) in procs {
        match proc.kind {
            ProcKind::Emerge => {
                res.start = std::cmp::min(res.start, proc.start);
                res.roots.push(*pid);
            },
            ProcKind::Python => {
                if let Some(a) = proc.cmdline.find("sandbox [") {
                    if let Some(b) = proc.cmdline.find("] sandbox") {
                        if let Some(p) = Pkg::try_new(&proc.cmdline[(a + 9)..b], false) {
                            res.pkgs.push(p);
                        }
                    }
                }
            },
            ProcKind::Other => (),
        }
    }
    // Remove roots that are (grand)children of another root
    if res.roots.len() > 1 {
        let origroots = res.roots.clone();
        res.roots.retain(|&r| {
                     let mut proc = procs.get(&r).expect("Root not in ProcList");
                     while let Some(p) = procs.get(&proc.ppid) {
                         if origroots.contains(&p.pid) {
                             debug!("Skipping proces {}: grandchild of {}", r, p.pid);
                             return false;
                         }
                         proc = p;
                     }
                     true
                 });
    }
    trace!("{:?}", res);
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::procs;

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
    fn check_resume(kind: ResumeKind, file: &str, expect: Option<&[(&str, bool)]>) {
        let expect_pkg: Option<Vec<Pkg>> =
            expect.map(|o| o.into_iter().map(|(s, b)| Pkg::try_new(s, *b).unwrap()).collect());
        let res = Mtimedb::try_new(&format!("tests/{file}")).and_then(|m| try_get_resume(kind, &m));
        assert_eq!(expect_pkg, res, "Mismatch for {file}");
    }

    #[test]
    fn resume() {
        let main = &[("dev-lang/rust-1.65.0", false), ("app-portage/emlop-0.5.0", false)];
        let bkp =
            &[("app-portage/dummybuild-0.1.600", false), ("app-portage/dummybuild-0.1.60", false)];
        let bin = &[("sys-devel/clang-19", false), ("www-client/falkon-24.08.3", true)];
        check_resume(ResumeKind::Main, "mtimedb.ok", Some(main));
        check_resume(ResumeKind::Backup, "mtimedb.ok", Some(bkp));
        check_resume(ResumeKind::No, "mtimedb.ok", Some(&[]));
        check_resume(ResumeKind::Either, "mtimedb.ok", Some(main));
        check_resume(ResumeKind::Either, "mtimedb.backuponly", Some(bkp));
        check_resume(ResumeKind::Either, "mtimedb.empty", None);
        check_resume(ResumeKind::Either, "mtimedb.mainempty", Some(bkp));
        check_resume(ResumeKind::Either, "mtimedb.noresume", None);
        check_resume(ResumeKind::Either, "mtimedb.badjson", None);
        check_resume(ResumeKind::Either, "mtimedb.binaries", Some(bin));
    }

    #[test]
    fn pkg_new() {
        assert_eq!(Some(Pkg { key: String::from("foo-1.2"), pos: 4, bin: true }),
                   Pkg::try_new("foo-1.2", true));
        assert_eq!(None, Pkg::try_new("foo1.2", true));
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

    /// Check that get_emerge() finds the expected roots
    #[test]
    fn get_emerge_roots() {
        let _ = env_logger::try_init();
        let procs = procs(&[(ProcKind::Emerge, "a", 1, 0),
                            (ProcKind::Other, "a.a", 2, 1),
                            (ProcKind::Emerge, "a.a.b", 3, 2),
                            (ProcKind::Other, "b", 4, 0),
                            (ProcKind::Emerge, "b.a", 5, 4),
                            (ProcKind::Other, "b.a.a", 6, 5)]);
        let einfo = get_emerge(&procs);
        assert_eq!(einfo.roots, vec![1, 5]);
    }

    #[test]
    fn pkgmoves() {
        // It's interesting to run this test with RUST_LOG=trace. Expect:
        // * "Cannot open tests/notfound: No such file or directory"
        // * "Using default sort ..." (depending on random hashmap seed)
        // * "Using move chain/v1 -> chain/v3 instead -> chain/v2 in tests/4Q-2022"
        // * "Ignoring move loop/final -> loop/from in tests/4Q-2022"
        let _ = env_logger::try_init();
        let moves = PkgMoves::new(&Mtimedb::try_new("tests/mtimedb.updates").unwrap());
        for (have, want, why) in
            [// Basic cases
             ("app-doc/doxygen", "app-text/doxygen", "simple move in 2024"),
             ("x11-libs/libva", "media-libs/libva", "simple move in 2022"),
             ("notmoved", "notmoved", "unknown string should return original string"),
             ("dev-haskell/extra", "dev-haskell/extra", "slotmoves should be ignored"),
             // Multi-moves where portage updated the old file
             ("dev-util/lldb", "llvm-core/lldb", "1st lldb rename"),
             ("dev-debug/lldb", "llvm-core/lldb", "2nd lldb rename"),
             // Weird cases
             ("duplicate/bar", "foo/bar", "duplicate update should prefer newest (no trace)"),
             ("conflict/foo", "foo/2024", "conflicting update should prefer newest (no trace)"),
             ("loop/from", "loop/final", "loops should prefer newest (trace \"ignore move...\")"),
             ("chain/v2", "chain/v3", "chain from new should be taken as-is (no trace)"),
             ("chain/v1",
              "chain/v3",
              "chain from old should point to new (trace \"using move...\")")]
        {
            assert_eq!(moves.get(String::from(have)), String::from(want), "{why}");
        }
    }
}
