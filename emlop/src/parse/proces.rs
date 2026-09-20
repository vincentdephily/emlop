//! This module extracts just enough info about running processes for emlop's usecase.
//!
//! It would have been natural to use the procinfo crate, but it fails parsing kernel processes, it
//! somehow yields start_times that can be off by a few seconds compared to this implem and the ps
//! program, and it probably parses a bit more than we need. So this is a handmade Linux-only
//! implementaion (does procinfo crate work on BSDs ?), but it's unit-tested against ps and should
//! be fast.

use crate::{config::Conf, table::Disp, wtb};
use emlop_lib::Proc;
use std::io::Write as _;

/// Like `Path.file_name()`, but less likely to interpret package categ/name as files
fn approx_filename(s: &str) -> Option<usize> {
    if s.chars().all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '/')) {
        s.rfind('/')
    } else {
        None
    }
}

pub struct FmtProc<'a>(/// process
                       pub &'a Proc,
                       /// Indent
                       pub usize,
                       /// Width
                       pub usize);
impl Disp for FmtProc<'_> {
    fn out(&self, buf: &mut Vec<u8>, gc: &Conf) -> usize {
        let FmtProc(Proc { cmdline, pid, .. }, indent, width) = *self;
        let (cnt, clr) = (gc.cnt.val, gc.clr.val);

        // Skip path and interpreter from command line
        let mut cmdstart = 0;
        if let Some(z1) = cmdline.find('\0') {
            if let Some(z2) = cmdline[z1 + 1..].find('\0')
               && let Some(f2) = approx_filename(&cmdline[z1 + 1..z1 + 1 + z2])
            {
                cmdstart = z1 + f2 + 2;
            } else if let Some(f1) = approx_filename(&cmdline[..z1]) {
                cmdstart = f1 + 1;
            }
        }
        let cmd = cmdline[cmdstart..].replace(|c: char| c.is_control(), " ");
        let cmd = cmd.trim();

        // Figure out how much space we have
        let pidlen = pid.max(&1).ilog10() as usize + 2 * indent + 1;
        let cmdcap = width.saturating_sub(pidlen + 1);

        // Output it
        if cmdcap >= cmd.len() {
            wtb!(buf, "{cnt}{pid:pidlen$}{clr} {cmd}");
            pidlen + 1 + cmd.len()
        } else if cmdcap > 3 {
            wtb!(buf, "{cnt}{pid:pidlen$}{clr} ...{}", &cmd[(cmd.len() - cmdcap + 3)..]);
            pidlen + 1 + cmdcap
        } else {
            wtb!(buf, "{cnt}{pid:pidlen$}{clr} ...");
            pidlen + 4
        }
    }
}


#[cfg(test)]
pub mod tests {
    use super::*;
    use emlop_lib::{Proc, ProcKind};

    /// FmtProc should shorten (elipsis at start) the command line when there is no space
    #[test]
    fn proc_width() {
        let conf = Conf::from_str(&format!("emlop p --color=n"));
        let t: Vec<_> = vec![// Here we have enough space
                             (1, "1", "1 1"),
                             (1, "12", "1 12"),
                             (1, "12345678", "1 12345678"),
                             (12, "1234567", "12 1234567"),
                             (123, "123456", "123 123456"),
                             (1234, "12345", "1234 12345"),
                             // Running out of space, but we can display part of it
                             (1, "1234567890", "1 ...67890"),
                             (12345, "1234567890", "12345 ...0"),
                             // Capacity is way too small, use elipsis starting at 4 chars
                             (1234567, "123", "1234567 ..."),
                             (123456, "123", "123456 123"),];
        for (pid, cmd, out) in t.into_iter() {
            let mut buf = vec![];
            let p = Proc { kind: ProcKind::Other, pid, ppid: 1, cmdline: cmd.into(), start: 0 };
            FmtProc(&p, 0, 10).out(&mut buf, &conf);
            assert_eq!(&String::from_utf8(buf).unwrap(),
                       out,
                       "got left expected right {pid} {cmd:?}");
        }
    }

    /// FmtProc should rewrite commands
    #[test]
    fn proc_cmdline() {
        let conf = Conf::from_str(&format!("emlop p --color=n"));
        let t: Vec<_> =
            vec![("foo\0bar", "1 foo bar"),
                 ("foo\0bar\0", "1 foo bar"),
                 ("/usr/bin/bash\0toto", "1 bash toto"),
                 ("/usr/bin/bash\0toto\0", "1 bash toto"),
                 ("/usr/bin/bash\0toto\0--arg", "1 bash toto --arg"),
                 ("/usr/bin/bash\0/path/to/toto\0--arg", "1 toto --arg"),
                 ("bash\0/usr/lib/portage/python3.12/ebuild.sh\0unpack\0", "1 ebuild.sh unpack"),
                 ("[foo/bar-0.1.600] sandbox\0/path/to/ebuild.sh\0unpack\0", "1 ebuild.sh unpack"),
                 ("[foo/bar-0.1.600] sandbox\0blah\0", "1 [foo/bar-0.1.600] sandbox blah"),
                 ("/bin/foo\0\0", "1 foo")];
        for (cmd, out) in t.into_iter() {
            let mut buf = vec![];
            let p = Proc { kind: ProcKind::Other, pid: 1, ppid: 1, cmdline: cmd.into(), start: 0 };
            FmtProc(&p, 0, 100).out(&mut buf, &conf);
            assert_eq!(&String::from_utf8(buf).unwrap(), out, "got left expected right {cmd:?}");
        }
    }
}
