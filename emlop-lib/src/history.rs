//! Handles emerge.log parsing.

use crate::{FmtUtc, HistBound};
use atoi::FromRadix10;
use flate2::read::GzDecoder;
use glob::glob;
use log::*;
use memchr::{memchr, memrchr, memrchr2};
use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use snafu::prelude::*;
use std::{fs::File,
          io::{BufRead, BufReader},
          path::PathBuf,
          str::from_utf8,
          sync::mpsc::{Receiver, sync_channel},
          thread,
          time::Instant};

/// Default location of portage `emerge.log` file
pub const DEFAULT_EMERGELOG: &str = "/var/log/emerge.log";

/// A parsed `emerge.log` line
///
/// Each variant corresponds to one interesting log line. The `key` can normaly be used to match
/// related lines together (the ones with the same prefix).
///
/// See [ELog].
#[derive(Debug)]
pub enum ELogLine {
    /// Emerge run started (might never complete).
    ///
    /// There's no RunStop, because matching a Stop to the correct Start is too unreliable
    RunStart { ts: i64, args: String },
    /// Merge started (might never complete).
    MergeStart { ts: i64, key: String, pos: usize },
    /// Merge is a binary merge.
    MergeBin { ts: i64, key: String, pos: usize },
    /// Merge completed.
    MergeStop { ts: i64, key: String, pos: usize },
    /// Unmerge started (might never complete).
    UnmergeStart { ts: i64, key: String, pos: usize },
    /// Unmerge completed.
    UnmergeStop { ts: i64, key: String, pos: usize },
    /// Sync started (might never complete).
    SyncStart { ts: i64 },
    /// Sync completed.
    SyncStop { ts: i64, repo: String },
}
impl ELogLine {
    /// Get the ebuild as an `&str`
    ///
    /// Panics for `Run` and `Sync` variants
    pub fn ebuild(&self) -> &str {
        match self {
            Self::MergeStart { key, pos, .. }
            | Self::MergeBin { key, pos, .. }
            | Self::MergeStop { key, pos, .. }
            | Self::UnmergeStart { key, pos, .. }
            | Self::UnmergeStop { key, pos, .. } => &key[..(*pos - 1)],
            _ => unreachable!("No ebuild for {:?}", self),
        }
    }
    /// Get the ebuild as an owned `String` (existing allocation), consuming self
    ///
    /// Panics for `Run` and `Sync` variants
    pub fn take_ebuild(self) -> String {
        match self {
            Self::MergeStart { mut key, pos, .. }
            | Self::MergeBin { mut key, pos, .. }
            | Self::MergeStop { mut key, pos, .. }
            | Self::UnmergeStart { mut key, pos, .. }
            | Self::UnmergeStop { mut key, pos, .. } => {
                key.truncate(pos - 1);
                key
            },
            _ => unreachable!("No ebuild for {:?}", self),
        }
    }
    #[cfg(test)]
    /// Get the version as an `&str`
    ///
    /// Panics for `Run` and `Sync` variants
    pub fn version(&self) -> &str {
        match self {
            Self::MergeStart { key, pos, .. }
            | Self::MergeBin { key, pos, .. }
            | Self::MergeStop { key, pos, .. }
            | Self::UnmergeStart { key, pos, .. }
            | Self::UnmergeStop { key, pos, .. } => &key[*pos..],
            _ => unreachable!("No version for {:?}", self),
        }
    }
    /// Get ebuild-version as an `&str`
    ///
    /// Panics for `Run` and `Sync` variants
    pub fn ebuild_version(&self) -> &str {
        match self {
            Self::MergeStart { key, .. }
            | Self::MergeBin { key, .. }
            | Self::MergeStop { key, .. }
            | Self::UnmergeStart { key, .. }
            | Self::UnmergeStop { key, .. } => key,
            _ => unreachable!("No ebuild/version for {:?}", self),
        }
    }
    /// Get the unix timestamp
    pub const fn ts(&self) -> i64 {
        match self {
            Self::RunStart { ts, .. }
            | Self::MergeStart { ts, .. }
            | Self::MergeBin { ts, .. }
            | Self::MergeStop { ts, .. }
            | Self::UnmergeStart { ts, .. }
            | Self::UnmergeStop { ts, .. }
            | Self::SyncStart { ts, .. }
            | Self::SyncStop { ts, .. } => *ts,
        }
    }
    /// Return the command kind
    ///
    /// Note that portage doesn't log all command kinds.
    ///
    /// Panics on variants other than `Run`.
    pub fn cmd_kind(&self) -> CmdKind {
        match self {
            Self::RunStart { args, .. } => {
                for arg in args.split_ascii_whitespace() {
                    match arg {
                        "--deselect" | "--unmerge" | "--clean" | "--depclean" => {
                            return CmdKind::Clean;
                        },
                        "--sync" => return CmdKind::Sync,
                        _ => (),
                    }
                }
                CmdKind::Merge
            },
            _ => unreachable!("No ebuild/version for {:?}", self),
        }
    }
}

/// Kind of logged emerge command
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum CmdKind {
    Merge,
    Clean,
    Sync,
}

/// Possible errors when constructing an [ELog]
#[derive(Snafu, Debug)]
#[snafu(context(suffix(Err)))]
pub enum ELogError {
    #[snafu(display("{op} {file:?}"))]
    /// Standard IO error
    IO { op: &'static str, file: PathBuf, source: std::io::Error },
    #[snafu(display("{file:?}: {msg}"))]
    /// Invalid file format
    File { msg: &'static str, file: PathBuf },
    /// Invalid regex
    #[snafu(transparent)]
    Regex { source: regex::Error },
    #[snafu(transparent)]
    Pattern { source: glob::PatternError },
    #[snafu(transparent)]
    Glob { source: glob::GlobError },
    #[snafu(display("{msg}"))]
    /// Bad argument
    Arg { msg: &'static str },
}

type FileBuf = BufReader<Box<dyn std::io::Read + Send>>;

/// Open maybe-compressed file, returning a BufReader
fn open_any_buffered(file: &PathBuf) -> Result<FileBuf, ELogError> {
    let reader = File::open(file).context(IOErr { op: "Open", file })?;
    if file.extension().is_some_and(|e| e == "gz") {
        let gz = GzDecoder::new(reader);
        ensure!(gz.header().is_some(), FileErr { msg: "invalid gzip header", file });
        Ok(BufReader::new(Box::new(gz)))
    } else {
        Ok(BufReader::new(Box::new(reader)))
    }
}

/// Parser for emerge log file
///
/// 1. Create a new parser with [new()](Self::new())
/// 2. Call the filter methodss if needed
/// 3. Consume explicitly with [into_iter()](Self::into_iter()), or implicitly using a `for` loop
pub struct ELog {
    srcs: Vec<(PathBuf, i64, FileBuf)>,
    min: i64,
    max: i64,
    merge: bool,
    unmerge: bool,
    run: bool,
    sync: bool,
    filter: Filter,
}
impl ELog {
    /// Create a new `emerge.log` file parser
    ///
    /// Pass [&\[DEFAULT_EMERGELOG\]](DEFAULT_EMERGELOG) if unsure.  
    /// Compressed files (*.gz) and [glob patterns](glob::Pattern) are supported.  
    /// File order is inferred from the first timestamp in each file.
    pub fn new(files: &[impl AsRef<str> + std::fmt::Debug]) -> Result<Self, ELogError> {
        let mut srcs = Vec::with_capacity(files.len());
        for f in files.iter().map(|f| f.as_ref()) {
            let paths = if f.contains(['*', '?', '[']) {
                glob(f)?.filter_map(Result::ok).collect()
            } else {
                vec![PathBuf::from(f)]
            };
            if paths.is_empty() {
                warn!("glob pattern {f} does not match any file");
            }
            for p in paths {
                if srcs.iter().all(|(sp, _, _)| *sp != p) {
                    let mut buf = open_any_buffered(&p)?;
                    match Self::peek_ts(&mut buf) {
                        Some(ts) => {
                            debug!("{p:?} starts at {}", FmtUtc(ts));
                            srcs.push((p, ts, buf));
                        },
                        None => return Err(ELogError::File { msg: "Bad emerge logfile", file: p }),
                    }
                }
            }
        }
        ensure!(!srcs.is_empty(), ArgErr { msg: "Expected at least one logfile" });
        srcs.sort_by_key(|s| s.1);
        Ok(Self { srcs,
                  min: i64::MIN,
                  max: i64::MAX,
                  merge: true,
                  unmerge: false,
                  run: false,
                  sync: false,
                  filter: Filter::True })
    }
    /// Parse the file only between `min` and `max`
    pub fn bounds(mut self, min: HistBound, max: HistBound) -> Result<Self, ELogError> {
        // FIXME look at all files
        (self.min, self.max) = filter_ts(&self.srcs, min, max)?;
        if self.min > self.max {
            std::mem::swap(&mut self.min, &mut self.max);
        }
        Ok(self)
    }
    /// Parse merge events
    pub fn merge(mut self, b: bool) -> Self {
        self.merge = b;
        self
    }
    /// Parse unmerge events
    pub fn unmerge(mut self, b: bool) -> Self {
        self.unmerge = b;
        self
    }
    /// Parse sync events
    pub fn sync(mut self, b: bool) -> Self {
        self.sync = b;
        self
    }
    /// Parse run events
    pub fn run(mut self, b: bool) -> Self {
        self.run = b;
        self
    }
    /// Filter events by ebuild or repo name
    ///
    /// * If exact=false: Case-insensitive regex matche on `repo` or `categ/ebuild`
    /// * If exact=true: String equality on `repo`, `categ/ebuild`, or `ebuild`
    pub fn search(mut self, terms: &Vec<String>, exact: bool) -> Result<Self, ELogError> {
        self.filter = Filter::try_new(terms, exact)?;
        Ok(self)
    }

    /// Prefill the buffer without advancing it, and look for a timestamp
    ///
    /// If no timestamp found in the first buffer-full or 20 lines, consider the file invalid.
    fn peek_ts(buffer: &mut FileBuf) -> Option<i64> {
        let buf = buffer.fill_buf().ok()?;
        let mut pos = 0;
        for _line in 1..=20 {
            let (num, next) = i64::from_radix_10(&buf[pos..]);
            if next > 0 && buf.get(pos + next) == Some(&b':') {
                return Some(num);
            }
            match buf[pos..].iter().position(|b| *b == b'\n') {
                Some(nl) => pos += nl + 1,
                None => break,
            }
        }
        warn!("No portage timestamp in the first {pos} bytes");
        None
    }

    /// Start the parsing thread for the given file/buf
    fn start_parse(&mut self,
                   file: PathBuf,
                   mut buf: FileBuf)
                   -> (thread::JoinHandle<()>, Receiver<ELogLine>) {
        let now = Instant::now();
        let filter = self.filter.clone();
        let Self { min, max, merge, unmerge, run, sync, .. } = *self;
        let (tx, rx) = sync_channel(256);
        let jh = thread::spawn(move || {
            let mut prev_t = 0;
            let mut curline = 1;
            let mut line = Vec::with_capacity(255);
            loop {
                match buf.read_until(b'\n', &mut line) {
                    // End of file
                    Ok(0) => break,
                    // Got a line, see if one of the funs match it
                    Ok(_) => {
                        if let Some((t, s)) = parse_ts(&line, min, max) {
                            if prev_t > t {
                                warn!("{file:?}:{curline}: System clock jump: {} -> {}",
                                      FmtUtc(prev_t),
                                      FmtUtc(t));
                            }
                            prev_t = t;
                            let parsed = parse_mergestart(merge, t, s, &filter)
                            .or_else(|| parse_mergestop(merge, t, s, &filter))
                            .or_else(|| parse_unmergestart(unmerge, t, s, &filter))
                            .or_else(|| parse_unmergestop(unmerge, t, s, &filter))
                            .or_else(|| parse_runstart(run, t, s))
                            .or_else(|| parse_syncstart(sync, t, s))
                            .or_else(|| parse_syncstop(sync, t, s, &filter))
                            .or_else(|| parse_mergebin(merge, t, s, &filter));
                            if let Some(found) = parsed
                               && tx.send(found).is_err()
                            {
                                break;
                            }
                        }
                    },
                    // System read error...
                    Err(e) => warn!("{file:?}:{curline}: {e}"),
                }
                line.clear();
                curline += 1;
            }
            debug!("Parsed {curline} {file:?} lines in {:?}", now.elapsed());
        });
        (jh, rx)
    }
}
impl IntoIterator for ELog {
    type Item = ELogLine;
    type IntoIter = ELogIter;
    /// Start parsing threads, and turn [ELog] into an [ELogIter]
    fn into_iter(mut self) -> ELogIter {
        let mut srcs = Vec::with_capacity(self.srcs.len());
        while let Some((file, ts, buf)) = self.srcs.pop() {
            let (jh, rx) = self.start_parse(file.clone(), buf);
            srcs.push((file, ts, jh, rx));
        }
        ELogIter { srcs, prev_t: 0 }
    }
}

/// Iterator for [ELogLine]s
pub struct ELogIter {
    srcs: Vec<(PathBuf, i64, thread::JoinHandle<()>, Receiver<ELogLine>)>,
    prev_t: i64,
}
impl Iterator for ELogIter {
    type Item = ELogLine;
    fn next(&mut self) -> Option<ELogLine> {
        loop {
            let (_, _, _, rx) = self.srcs.last()?;
            if let Ok(l) = rx.recv() {
                self.prev_t = l.ts();
                return Some(l);
            } else {
                let (prev_f, _, jh, _) = self.srcs.pop()?;
                if let Err(e) = jh.join() {
                    error!("Panic parsing {prev_f:?} after {}. Please report this on the emlop repo",
                           self.prev_t);
                    std::panic::resume_unwind(e);
                }
                if let Some((next_f, next_t, _, _)) = self.srcs.last()
                   && self.prev_t > *next_t
                {
                    warn!("Timestamp overlap at {prev_f:?}:{} -> {next_f:?}:{}",
                          FmtUtc(self.prev_t),
                          FmtUtc(*next_t));
                }
            }
        }
    }
}


/// Return min/max timestamp depending on options.
fn filter_ts(srcs: &[(PathBuf, i64, FileBuf)],
             min: HistBound,
             max: HistBound)
             -> Result<(i64, i64), ELogError> {
    // Parse emerge logs (expected to be in chronological order) into a Vec of emerge runs.
    // This uses a cut-down parser, about 20% faster than start_parse() for this usecase
    let mut runs = vec![];
    if matches!(min, HistBound::RunFirst(_) | HistBound::RunLast(_))
       || matches!(max, HistBound::RunFirst(_) | HistBound::RunLast(_))
    {
        for (file, _, _) in srcs {
            let mut buf = open_any_buffered(file)?;
            let mut line = Vec::with_capacity(255);
            loop {
                match buf.read_until(b'\n', &mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Some((t, s)) = parse_ts(&line, i64::MIN, i64::MAX)
                           && s.starts_with(b"*** emerge")
                        {
                            runs.push(t)
                        }
                    },
                    Err(_) => (),
                }
                line.clear();
            }
        }
    }
    // Convert to Option<int>
    let min = match min {
        HistBound::RunFirst(n) => runs.get(n).copied(),
        HistBound::RunLast(n) => runs.iter().rev().nth(n).copied(),
        HistBound::Unix(n) => Some(n),
        HistBound::None => None,
    };
    let max = match max {
        HistBound::RunFirst(n) => runs.get(n).copied(),
        HistBound::RunLast(n) => runs.iter().rev().nth(n).copied(),
        HistBound::Unix(n) => Some(n),
        HistBound::None => None,
    };
    // Check and log bounds, return result
    match (min, max) {
        (None, None) => trace!("Date: None"),
        (Some(a), None) => trace!("Date: after {}", FmtUtc(a)),
        (None, Some(b)) => trace!("Date: before {}", FmtUtc(b)),
        (Some(a), Some(b)) => trace!("Date: between {} and {}", FmtUtc(b), FmtUtc(a)),
    }
    Ok((min.unwrap_or(i64::MIN), max.unwrap_or(i64::MAX)))
}

#[derive(Clone)]
/// Matches package/repo depending on options.
enum Filter {
    True,
    Eq { a: Vec<String>, b: Vec<String>, c: Vec<String> },
    Re1 { r: Regex },
    Re { r: RegexSet },
}
impl Filter {
    fn try_new(terms: &Vec<String>, exact: bool) -> Result<Self, regex::Error> {
        trace!("Search: {terms:?} {exact}");
        Ok(match (terms.len(), exact) {
            (0, _) => Self::True,
            (_, true) => {
                let (b, c) = terms.iter().cloned().partition(|s| s.contains('/'));
                Self::Eq { a: terms.clone(),
                           b,
                           c: c.into_iter().map(|s| format!("/{s}")).collect() }
            },
            (1, false) => {
                Self::Re1 { r: RegexBuilder::new(&terms[0]).case_insensitive(true).build()? }
            },
            (_, false) => {
                Self::Re { r: RegexSetBuilder::new(terms).case_insensitive(true).build()? }
            },
        })
    }
    fn match_pkg(&self, s: &str) -> bool {
        match &self {
            Self::True => true,
            Self::Eq { b, c, .. } => b.iter().any(|e| e == s) || c.iter().any(|e| s.ends_with(e)),
            Self::Re1 { r } => r.is_match(s),
            Self::Re { r } => r.is_match(s),
        }
    }
    fn match_str(&self, s: &str) -> bool {
        match &self {
            Self::True => true,
            Self::Eq { a, .. } => a.iter().any(|e| e == s),
            Self::Re1 { r } => r.is_match(s),
            Self::Re { r } => r.is_match(s),
        }
    }
}


/// Find position of "version" in "categ/name-version" and filter on pkg name
fn parse_version(atom: &str, filter: &Filter) -> Option<usize> {
    let batom = atom.as_bytes();
    let mut pos = 0;
    loop {
        pos += memchr(b'-', &batom[pos..])? + 1;
        if pos > 1 && batom.get(pos)?.is_ascii_digit() {
            return filter.match_pkg(&atom[..(pos - 1)]).then_some(pos);
        }
    }
}

/// Parse and filter timestamp, trim line
fn parse_ts(line: &[u8], min: i64, max: i64) -> Option<(i64, &[u8])> {
    match i64::from_radix_10(line) {
        (ts, n) if n != 0 && ts >= min && ts <= max && line.get(n) == Some(&b':') => {
            let mut line = &line[(n + 1)..];
            while let [b' ', rest @ ..] = line {
                line = rest;
            }
            if let [rest @ .., b'\n'] = line {
                line = rest;
            }
            Some((ts, line))
        },
        _ => None,
    }
}

/// *** emerge --update --ask --deep --reinstall=changed-use --regex-search-auto=y --verbose system
fn parse_runstart(enabled: bool, ts: i64, line: &[u8]) -> Option<ELogLine> {
    if !enabled || !line.starts_with(b"*** emer") {
        return None;
    }
    Some(ELogLine::RunStart { ts, args: from_utf8(&line[11..]).ok()?.to_owned() })
}

/// >>> emerge (1 of 1) www-client/falkon-24.08.3 to /
fn parse_mergestart(enabled: bool, ts: i64, line: &[u8], filter: &Filter) -> Option<ELogLine> {
    if !enabled || !line.starts_with(b">>> emer") {
        return None;
    }
    let mut tokens = line.split(|c| *c == b' ');
    let atom = from_utf8(tokens.nth(5)?).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(ELogLine::MergeStart { ts, key: atom.to_owned(), pos })
}

/// === (1 of 1) Merging Binary (www-client/falkon-24.08.3::/var/cache/binpkgs/www-client/falkon-24.08.3.gpkg.tar)
fn parse_mergebin(enabled: bool, ts: i64, line: &[u8], filter: &Filter) -> Option<ELogLine> {
    if !enabled || !line.starts_with(b"=== (") {
        return None;
    }
    let p1 = memchr(b')', line)?;
    if !&line[(p1 + 2)..].starts_with(b"Merging Bin") {
        return None;
    }
    let p2 = memchr(b'(', &line[p1..])? + p1 + 1;
    let p3 = memchr(b':', &line[p2..])? + p2;
    let atom = from_utf8(&line[p2..p3]).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(ELogLine::MergeBin { ts, key: atom.to_owned(), pos })
}

/// ::: completed emerge (1 of 1) www-client/falkon-24.08.3 to /
fn parse_mergestop(enabled: bool, ts: i64, line: &[u8], filter: &Filter) -> Option<ELogLine> {
    if !enabled || !line.starts_with(b"::: comp") {
        return None;
    }
    let mut tokens = line.split(|c| *c == b' ');
    let atom = from_utf8(tokens.nth(6)?).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(ELogLine::MergeStop { ts, key: atom.to_owned(), pos })
}

/// === Unmerging... (app-portage/getuto-1.13)
fn parse_unmergestart(enabled: bool, ts: i64, line: &[u8], filter: &Filter) -> Option<ELogLine> {
    if !enabled || !line.starts_with(b"=== Unmer") {
        return None;
    }
    let p1 = memchr(b'(', line)? + 1;
    let atom = from_utf8(&line[p1..line.len() - 1]).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(ELogLine::UnmergeStart { ts, key: atom.to_owned(), pos })
}

/// >>> unmerge success: www-client/falkon-24.08.3
fn parse_unmergestop(enabled: bool, ts: i64, line: &[u8], filter: &Filter) -> Option<ELogLine> {
    if !enabled || !line.starts_with(b">>> unmerge succ") {
        return None;
    }
    let p1 = memrchr(b' ', line)? + 1;
    let atom = from_utf8(&line[p1..]).ok()?;
    let pos = parse_version(atom, filter)?;
    Some(ELogLine::UnmergeStop { ts, key: atom.to_owned(), pos })
}

/// >>> Syncing repository 'gentoo' into '/usr/portage'...
/// >>> Starting rsync with rsync://91.186.30.235/gentoo-portage
fn parse_syncstart(enabled: bool, ts: i64, line: &[u8]) -> Option<ELogLine> {
    // Old portage logs 'Starting rsync with <url>', new portage logs 'Syncing repository <name>',
    // and intermediate versions log both. This makes it hard to properly match a start repo string
    // to a stop repo string across portage versions. Since syncs are not concurrent, we simply
    // ignore the start repo string.
    if enabled
       && (line.starts_with(b">>> Syncing")
           || line.starts_with(b">>> Starting rsync")
           || line.starts_with(b">>> starting rsync"))
    {
        Some(ELogLine::SyncStart { ts })
    } else {
        None
    }
}

/// === Sync completed with rsync://209.177.148.226/gentoo-portage
/// === Sync completed for gentoo
fn parse_syncstop(enabled: bool, ts: i64, line: &[u8], filter: &Filter) -> Option<ELogLine> {
    // Old portage logs 'completed with <url>', new portage logs 'completed for <name>'
    if !enabled || !line.starts_with(b"=== Sync comp") {
        return None;
    }
    let pos = memrchr2(b' ', b'/', line)? + 1;
    let repo = from_utf8(&line[pos..]).ok()?;
    filter.match_str(repo).then_some(ELogLine::SyncStop { ts, repo: repo.to_owned() })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parse_ts_() {
        for (s, r) in vec![("", None as Option<(i64, &[u8])>),
                           ("\n", None),
                           ("123", None),
                           ("123\n", None),
                           ("123A", None),
                           ("123A:", None),
                           ("123:", Some((123, b""))),
                           ("123:\n", Some((123, b""))),
                           ("123:  \n", Some((123, b""))),
                           ("123:boo", Some((123, b"boo"))),
                           ("123:  boo", Some((123, b"boo"))),
                           ("123:  boo\n", Some((123, b"boo"))),
                           ("A123: boo", None),]
        {
            assert_eq!(parse_ts(s.as_bytes(), i64::MIN, i64::MAX), r, "parse_ts({s}) -> {r:?}");
        }
    }

    /// This checks parsing the given emerge.log.
    fn chk_hist(file: &str,
                show: &str,
                filter_mints: Option<i64>,
                filter_maxts: Option<i64>,
                filter_terms: Vec<String>,
                exact: bool,
                expect_counts: Vec<(&str, usize)>) {
        // Setup
        let (mints, maxts) = match file {
            "10000" => (1517609348, 1520991402),
            "all" => (1483228800, 1483747200),
            "badversion" => (1327867709, 1327871057),
            "nullbytes" => (1327867709, 1327871057),
            "shortline" => (1327867709, 1327871057),
            o => unimplemented!("Unknown test log file {:?}", o),
        };
        let file = &[format!("tests/emerge.{file}.log")];
        let min = filter_mints.map_or(HistBound::None, HistBound::Unix);
        let max = filter_maxts.map_or(HistBound::None, HistBound::Unix);
        let elog = ELog::new(file).expect("el new")
                                  .bounds(min, max)
                                  .expect("el bounds")
                                  .merge(show.contains("m"))
                                  .unmerge(show.contains("u"))
                                  .run(show.contains("r"))
                                  .sync(show.contains("s"))
                                  .search(&filter_terms, exact)
                                  .expect("el search");
        let re_atom = Regex::new("^[a-zA-Z0-9-]+/[a-zA-Z0-9_+-]+$").unwrap();
        let re_version = Regex::new("^[0-9][0-9a-z._-]*$").unwrap();
        let mut counts: HashMap<String, usize> = HashMap::new();
        // Check that all items look valid
        for p in elog {
            let (kind, ts, ebuild, version) = match p {
                ELogLine::RunStart { ts, .. } => ("RStart", ts, "c/e", "1"),
                ELogLine::MergeStart { ts, .. } => ("MStart", ts, p.ebuild(), p.version()),
                ELogLine::MergeBin { ts, .. } => ("MBin", ts, p.ebuild(), p.version()),
                ELogLine::MergeStop { ts, .. } => ("MStop", ts, p.ebuild(), p.version()),
                ELogLine::UnmergeStart { ts, .. } => ("UStart", ts, p.ebuild(), p.version()),
                ELogLine::UnmergeStop { ts, .. } => ("UStop", ts, p.ebuild(), p.version()),
                ELogLine::SyncStart { ts, .. } => ("SStart", ts, "c/e", "1"),
                ELogLine::SyncStop { ts, .. } => ("SStop", ts, "c/e", "1"),
            };
            *counts.entry(kind.to_string()).or_insert(0) += 1;
            *counts.entry(ebuild.to_string()).or_insert(0) += 1;
            assert!(ts >= filter_mints.unwrap_or(mints) && ts <= filter_maxts.unwrap_or(maxts),
                    "Out of bound date {} in  in {p:?}",
                    FmtUtc(ts));
            assert!(re_atom.is_match(ebuild), "Invalid ebuild atom {} in {p:?}", ebuild);
            assert!(re_version.is_match(version), "Invalid version {} in {p:?}", version);
        }
        // Check that we got the right number of each kind
        for (t, ref c) in expect_counts {
            let v = counts.get(t).unwrap_or(&0);
            assert_eq!(v, c,
                       "Got {} {}, expected {:?} with pkg={:?} exact={} min={:?} max={:?}",
                       v, t, c, filter_terms, exact, filter_mints, filter_maxts);
        }
    }

    #[test]
    /// Simplified emerge log containing all the ebuilds in all the versions of the current portage tree (see testdata/generate.sh)
    fn parse_hist_all() {
        let t = vec![("MStart", 31467)];
        chk_hist("all", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_nullbytes() {
        let t = vec![("MStart", 14), ("MStop", 14)];
        chk_hist("nullbytes", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_badversion() {
        let t = vec![("MStart", 3),
                     ("MStop", 2),
                     ("media-libs/jpeg", 2),
                     ("dev-libs/libical", 2),
                     ("media-libs/libpng", 1)]; //missing version
        chk_hist("badversion", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Emerge log with various invalid data
    fn parse_hist_shortline() {
        let t = vec![("MStart", 3),
                     ("MStop", 2),
                     ("media-libs/jpeg", 2),
                     ("dev-libs/libical", 1), //missing end of line and spaces in iter
                     ("media-libs/libpng", 2)];
        chk_hist("shortline", "m", None, None, vec![], false, t);
    }

    #[test]
    /// Basic counts, with every combination of command/merge/unmerge/sync
    fn parse_hist_nofilter() {
        for i in 0..16 {
            let r = (i & 0b0001) == 0;
            let m = (i & 0b0010) == 0;
            let u = (i & 0b0100) == 0;
            let s = (i & 0b1000) == 0;
            let show = format!("{}{}{}{}",
                               if r { "r" } else { "" },
                               if m { "m" } else { "" },
                               if u { "u" } else { "" },
                               if s { "s" } else { "" });
            let t = vec![("RStart", if r { 451 } else { 0 }),
                         ("MStart", if m { 890 } else { 0 }),
                         ("MBin", if m { 1 } else { 0 }),
                         ("MStop", if m { 833 } else { 0 }),
                         ("UStart", if u { 833 } else { 0 }),
                         ("UStop", if u { 833 } else { 0 }),
                         ("SStart", if s { 326 } else { 0 }),
                         ("SStop", if s { 150 } else { 0 })];
            chk_hist("10000", &show, None, None, vec![], false, t);
        }
    }

    #[test]
    /// Filtering by search term
    fn parse_hist_filter_term() {
        #[rustfmt::skip]
        let t = vec![
            ("",                           false, 890,  1, 833, 833, 833, 150), // Everything
            ("kactivities",                false,   4,  0,   4,   4,   4,   0), // regexp matches 4
            ("kactivities",                true,    2,  0,   2,   2,   2,   0), // string matches 2
            ("kde-frameworks/kactivities", true,    2,  0,   2,   2,   2,   0), // string matches 2
            ("frameworks/kactivities",     true,    0,  0,   0,   0,   0,   0), // string matches nothing
            ("ks/kw",                      false,   9,  0,   8,   8,   8,   0), // regexp matches 16 (+1 failed)
            ("file",                       false,   7,  0,   7,   6,   6,   0), // case-insensitive
            ("FILE",                       false,   7,  0,   7,   6,   6,   0), // case-insensitive
            ("file-next",                  true,    0,  0,   0,   0,   0,   0), // case-sensitive
            ("File-Next",                  true,    1,  0,   1,   0,   0,   0), // case-sensitive
            ("gentoo",                     true,    0,  0,   0,   0,   0, 150), // repo sync only
            ("gentoo",                     false,  11,  0,  11,  12,  12, 150), // repo and ebuilds
            ("ark oxygen",                 false,  15,  0,  15,  15,  15,   0), // multiple regex terms
            ("ark oxygen",                 true,    8,  0,   8,   8,   8,   0), // multiple string terms
        ];
        for (f, e, m1, m2, m3, u1, u2, s2) in t {
            let c = vec![("MStart", m1),
                         ("MBin", m2),
                         ("MStop", m3),
                         ("UStart", u1),
                         ("UStop", u2),
                         // SStart is always the same because Sync filtering is only done for SStop
                         ("SStart", 326),
                         ("SStop", s2)];
            let terms = f.split_whitespace().map(str::to_string).collect();
            chk_hist("10000", "mus", None, None, terms, e, c);
        }
    }

    #[test]
    /// Filtering by timestamp
    fn parse_hist_filter_ts() {
        let (umin, umax, fmin, fmax) = (i64::MIN, i64::MAX, 1517609348, 1520991402);
        #[rustfmt::skip]
        let t = vec![(Some(umin),       None,           890,   1, 833, 833, 833, 326, 150),
                     (Some(fmin),       None,           890,   1, 833, 833, 833, 326, 150),
                     (None,             Some(umax),     890,   1, 833, 833, 833, 326, 150),
                     (None,             Some(fmax),     890,   1, 833, 833, 833, 326, 150),
                     (Some(fmin),       Some(fmax),     890,   1, 833, 833, 833, 326, 150),
                     (Some(fmax),       None,             0,   0,   1,   0,   0,   0,   0), //last ts contains a stop and a "Post-Build Cleaning" step
                     (None,             Some(fmin),       0,   0,   1,   0,   0,   0,   0), //fist ts contains a stop
                     (None,             Some(umin),       0,   0,   0,   0,   0,   0,   0),
                     (Some(umax),       None,             0,   0,   0,   0,   0,   0,   0),
                     (Some(1517917751), Some(1517931835), 6,   0,   6,   5,   5,   4,   2),
                     (Some(1517959010), Some(1518176159), 24,  0,  21,  23,  23,  32,  16),
        ];
        for (min, max, m1, m2, m3, u1, u2, s1, s2) in t {
            let c = vec![("MStart", m1),
                         ("MBin", m2),
                         ("MStop", m3),
                         ("UStart", u1),
                         ("UStop", u2),
                         ("SStart", s1),
                         ("SStop", s2)];
            chk_hist("10000", "mus", min, max, vec![], true, c);
        }
    }

    #[test]
    /// Filtering by search term
    fn filter_terms() {
        let t = vec![("a", true, "a", false, true),
                     ("a", true, "b/a", true, false),
                     ("a", true, "aa", false, false),
                     ("a", true, "b/aa", false, false),
                     ("a.", true, "ab", false, false),
                     ("a.", false, "ab", true, true),];
        for (terms, e, s, mpkg, mstr) in t {
            let t: Vec<String> = terms.split_whitespace().map(str::to_string).collect();
            let f = Filter::try_new(&t, e).unwrap();
            assert_eq!(f.match_pkg(s), mpkg, "filter({t:?}, {e}).match_pkg({s:?})");
            assert_eq!(f.match_str(s), mstr, "filter({t:?}, {e}).match_str({s:?})");
        }
    }

    #[test]
    /// Filtering by run number (other HistBound variants are is tested in date.rs)
    fn filter_ts_run() {
        use HistBound::{RunFirst, RunLast};
        let f1 = PathBuf::from("tests/emerge.10000.log");
        let f2 = PathBuf::from("tests/emerge.dst.log");
        let t = vec![// base checks
                     (vec![&f1], RunFirst(0), RunLast(0), (1517698744, 1520991376)),
                     (vec![&f2], RunFirst(0), RunLast(0), (1616781118, 1617015378)),
                     (vec![&f1, &f2], RunFirst(0), RunLast(0), (1517698744, 1617015378)),
                     // RunLast(10) reaches past f2 and into f1
                     (vec![&f1, &f2], RunFirst(10), RunLast(10), (1517823042, 1520853735)),
                     // Going too far yields MIN/MAX
                     (vec![&f2], RunFirst(10), RunLast(10), (i64::MIN, i64::MAX))];
        for (files, argmin, argmax, expect) in t {
            let srcs: Vec<_> = files.into_iter()
                                    .map(|f| (f.clone(), 0i64, open_any_buffered(&f).unwrap()))
                                    .collect();
            let res = filter_ts(&srcs, argmin, argmax);
            assert_eq!(res.unwrap(), expect);
        }
    }

    #[test]
    fn split_atom() {
        let g = |s| parse_version(s, &Filter::True).map(|n| (&s[..n - 1], &s[n..]));
        assert_eq!(None, g(""));
        assert_eq!(None, g("a"));
        assert_eq!(None, g("-"));
        assert_eq!(None, g("42"));
        assert_eq!(None, g("-42"));
        assert_eq!(None, g("42-"));
        assert_eq!(None, g("a-/"));
        assert_eq!(Some(("a", "0")), g("a-0"));
        assert_eq!(Some(("a", "1")), g("a-1"));
        assert_eq!(Some(("a", "2")), g("a-2"));
        assert_eq!(Some(("a", "3")), g("a-3"));
        assert_eq!(Some(("a", "4")), g("a-4"));
        assert_eq!(Some(("a", "5")), g("a-5"));
        assert_eq!(Some(("a", "6")), g("a-6"));
        assert_eq!(Some(("a", "7")), g("a-7"));
        assert_eq!(Some(("a", "8")), g("a-8"));
        assert_eq!(Some(("a", "9")), g("a-9"));
        assert_eq!(None, g("a-:"));
        assert_eq!(Some(("a-b", "2")), g("a-b-2"));
        assert_eq!(Some(("a-b", "2-3")), g("a-b-2-3"));
        assert_eq!(Some(("a-b", "2-3_r1")), g("a-b-2-3_r1"));
        assert_eq!(Some(("a-b", "2foo-4")), g("a-b-2foo-4"));
        assert_eq!(Some(("a-b", "2foo-4-")), g("a-b-2foo-4-"));
        assert_eq!(Some(("Noël", "2-bêta")), g("Noël-2-bêta"));
    }

    #[test]
    fn peek_ts() {
        for (s, r) in [("", None),
                       ("\n", None),
                       ("123", None),
                       ("123:", Some(123)),
                       ("123:foo", Some(123)),
                       ("\n123", None),
                       ("\n123:foo", Some(123)),
                       ("\nfoo123", None),
                       ("foo123bar", None),
                       ("foo123bar\n456:\n789:", Some(456)),
                       ("\n\n\n123\n\n\n\n456:", Some(456))]
        {
            let mut b = BufReader::new(Box::new(s.as_bytes()) as Box<dyn std::io::Read + Send>);
            assert_eq!(r, ELog::peek_ts(&mut b), "{s:?}");
        }
    }

    #[test]
    /// Check order of ELogIter files (most recent first)
    fn elog_file_order() {
        let old = "tests/emerge.badversion.log";
        let mid = "tests/emerge.log.gz";
        let new = "tests/emerge.10000.log";
        let glob = "tests/emerge.[bl]*";
        let badglob = "tests/foo*";
        for (h, w) in [(vec![old], vec![old]),
                       (vec![old, new], vec![new, old]),
                       (vec![old, new, old, new], vec![new, old]),
                       (vec![new, old], vec![new, old]),
                       (vec![mid, old, new], vec![new, mid, old]),
                       (vec![glob], vec![mid, old]),
                       (vec![glob, mid], vec![mid, old]),
                       (vec![badglob, mid], vec![mid])]
        {
            let elog = ELog::new(&h).unwrap().into_iter();
            let have: Vec<_> = elog.srcs.into_iter().map(|(s, _, _, _)| s).collect();
            let want: Vec<_> = w.iter().map(PathBuf::from).collect();
            assert_eq!(have, want, "{h:?} -> {w:?}");
        }
    }

    #[test]
    /// Check ELog::new() file rejections
    fn elog_file_reject() {
        let good = "tests/emerge.10000.log";
        let notlog = "tests/1Q-2024";
        let notfound = "tests/notfound";
        let badglob = "tests/foo*";
        for h in [vec![],
                  vec![notlog],
                  vec![notfound],
                  vec![good, notlog],
                  vec![good, notfound],
                  vec![badglob]]
        {
            assert!(matches!(ELog::new(&h), Err(_)), "{h:?}");
        }
    }
}

#[cfg(feature = "unstable")]
#[cfg(test)]
mod bench {
    use super::*;
    use std::sync::LazyLock;
    extern crate test;

    fn bench_log() -> ELog {
        ELog::new(&["../benches/emerge.log"]).unwrap()
    }

    /// Vec<(full, no_ts)> of emerge.log lines
    static EMERGE_LINES: LazyLock<Vec<(&[u8], &[u8])>> = LazyLock::new(|| {
        let elog = include_str!("../../benches/emerge.log");
        elog.split('\n')
            .filter_map(|l| {
                let mut p = l.find(' ')?;
                while l.as_bytes().get(p) == Some(&b' ') {
                    p += 1;
                }
                Some((l.as_bytes(), l[p..].as_bytes()))
            })
            .collect()
    });

    /// Vec<String> of package categ/name-version
    static PKGS: LazyLock<Vec<String>> = LazyLock::new(|| {
        let f = |p| match p {
            ELogLine::MergeStart { key, .. } => Some(key),
            _ => None,
        };
        let pkgs: Vec<_> = bench_log().into_iter().filter_map(f).collect();
        assert_eq!(pkgs.len(), 10790);
        pkgs
    });

    macro_rules! bench_filterstr {
        ($n:ident, $t:expr, $e:expr) => {
            #[bench]
            /// Bench creating a filter and applying it on many strings
            fn $n(b: &mut test::Bencher) {
                let t: Vec<String> = $t.split_whitespace().map(str::to_string).collect();
                let pkgs = &*PKGS;
                b.iter(move || {
                     let f = Filter::try_new(&t, $e).unwrap();
                     pkgs.iter().fold(true, |a, p| a ^ f.match_pkg(&p))
                 });
            }
        };
    }
    bench_filterstr!(filterstr_none, "", true);
    bench_filterstr!(filterstr_one_str, "gcc", true);
    bench_filterstr!(filterstr_one_full, "virtual/rust", true);
    bench_filterstr!(filterstr_many_str, "gcc llvm clang rust emacs", true);
    bench_filterstr!(filterstr_one_reg, "gcc", false);
    bench_filterstr!(filterstr_many_reg, "gcc llvm clang rust emacs", false);

    #[bench]
    fn parse_version_(b: &mut test::Bencher) {
        let pkgs = &*PKGS;
        b.iter(move || {
             for p in pkgs {
                 parse_version(&p, &Filter::True);
             }
         });
    }

    /// Bench parsing a whole log file without filter or postprocessing
    fn get_hist_with(b: &mut test::Bencher, s: &str) {
        let want: usize = s.chars()
                           .map(|c| match c {
                               'm' => 21320,
                               'u' => 20847,
                               's' => 661,
                               'r' => 971,
                               o => panic!("unhandled show {o}"),
                           })
                           .sum();
        b.iter(move || {
             let have = bench_log().merge(s.contains("m"))
                                   .unmerge(s.contains("u"))
                                   .run(s.contains("r"))
                                   .sync(s.contains("s"))
                                   .into_iter()
                                   .count();
             assert_eq!(have, want);
         });
    }
    #[bench]
    fn get_hist_murs(b: &mut test::Bencher) {
        get_hist_with(b, "murs")
    }
    #[bench]
    fn get_hist_m(b: &mut test::Bencher) {
        get_hist_with(b, "m")
    }
    #[bench]
    fn get_hist_u(b: &mut test::Bencher) {
        get_hist_with(b, "u")
    }
    #[bench]
    fn get_hist_r(b: &mut test::Bencher) {
        get_hist_with(b, "r")
    }
    #[bench]
    fn get_hist_s(b: &mut test::Bencher) {
        get_hist_with(b, "s")
    }

    macro_rules! bench_parselines {
        ($n:ident, $f:expr) => {
            #[bench]
            fn $n(b: &mut test::Bencher) {
                let f = Filter::True;
                let lines = &*EMERGE_LINES;
                b.iter(move || {
                     let mut found = 0;
                     for (l1, l2) in lines {
                         found += $f(&f, l1, l2).is_some() as u32;
                     }
                     assert!(found > 2, "Only {found} matches for {}", stringify!($f));
                 });
            }
        };
    }
    bench_parselines!(parse_ts_, |_, s, _| parse_ts(s, i64::MIN, i64::MAX));
    bench_parselines!(parse_runstart_, |_, _, s| parse_runstart(true, 1, s));
    bench_parselines!(parse_mergestart_, |f, _, s| parse_mergestart(true, 1, s, f));
    bench_parselines!(parse_mergestop_, |f, _, s| parse_mergestop(true, 1, s, f));
    bench_parselines!(parse_mergebin_, |f, _, s| parse_mergebin(true, 1, s, f));
    bench_parselines!(parse_unmergestart_, |f, _, s| parse_unmergestart(true, 1, s, f));
    bench_parselines!(parse_unmergestop_, |f, _, s| parse_unmergestop(true, 1, s, f));
    bench_parselines!(parse_syncstart_, |_, _, s| parse_syncstart(true, 1, s));
    bench_parselines!(parse_syncstop_, |f, _, s| parse_syncstop(true, 1, s, f));
}
