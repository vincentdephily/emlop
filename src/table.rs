use crate::{Conf, OutStyle};
use std::{collections::VecDeque,
          io::{stdout, BufWriter, Write as _}};

pub trait Disp {
    /// Write to buf and returns the number of visible chars written
    fn out(&self, buf: &mut Vec<u8>, conf: &Conf) -> usize;
}
impl<T: std::fmt::Display> Disp for T {
    fn out(&self, buf: &mut Vec<u8>, _conf: &Conf) -> usize {
        let start = buf.len();
        write!(buf, "{self}").expect("write to buf");
        buf.len() - start
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

const SPACES: [u8; 512] = [b' '; 512];

pub struct Table<'a, const N: usize> {
    /// Buffer where unaligned entries are written
    ///
    /// We can only render alignments when we saw all the rows.
    /// Having a single buffer noticably speed things up by reducing allocations.
    buf: Vec<u8>,
    /// Visible length, and start/stop index into buffer
    rows: VecDeque<[(usize, usize, usize); N]>,
    /// Table header
    header: Option<[(usize, usize, usize); N]>,

    /// Main config
    conf: &'a Conf,
    /// Column alignments (defaults to Right)
    aligns: [Align; N],
    /// Margin between columns, printed left of the column, defaults to `"  "`
    margins: [&'static str; N],
    /// Only print last N rows
    last: usize,
}

impl<'a, const N: usize> Table<'a, N> {
    /// Initialize new table
    pub fn new(conf: &'a Conf) -> Table<'a, N> {
        Self { rows: VecDeque::with_capacity(32),
               buf: Vec::with_capacity(1024),
               conf,
               header: None,
               aligns: [Align::Right; N],
               margins: ["  "; N],
               last: usize::MAX }
    }

    /// Specify column alignment
    pub const fn align_left(mut self, col: usize) -> Self {
        self.aligns[col] = Align::Left;
        self
    }

    /// Specify column left margin (1st printted column never has a left margin)
    pub const fn margin(mut self, col: usize, margin: &'static str) -> Self {
        self.margins[col] = margin;
        self
    }

    /// Specify column left margin (1st printted column never has a left margin)
    pub const fn last(mut self, last: usize) -> Self {
        self.last = last;
        self
    }

    /// Add a section header
    pub fn header(&mut self, row: [&str; N]) {
        if self.conf.header {
            let mut idxrow = [(0, 0, 0); N];
            for i in 0..N {
                let start = self.buf.len();
                self.buf.extend(row[i].as_bytes());
                idxrow[i] = (row[i].len(), start, self.buf.len());
            }
            self.header = Some(idxrow);
        }
    }

    /// Is there actual data to flush ?
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Add one row of data
    ///
    /// The number of cells is set by const generic.
    /// Each cell is an array of displayables.
    pub fn row(&mut self, row: [&[&dyn Disp]; N]) {
        let mut idxrow = [(0, 0, 0); N];
        for i in 0..N {
            let start = self.buf.len();
            let len = row[i].iter().map(|c| c.out(&mut self.buf, self.conf)).sum();
            idxrow[i] = (len, start, self.buf.len());
        }
        self.rows.push_back(idxrow);
        if self.rows.len() > self.last {
            self.rows.pop_front();
        }
    }

    fn flush(&self, mut out: impl std::io::Write) {
        if self.is_empty() {
            return;
        }
        // Check the max len of each column, for the rows we have
        let widths: [usize; N] = std::array::from_fn(|i| {
            self.rows.iter().chain(self.header.iter()).fold(0, |m, r| usize::max(m, r[i].0))
        });
        if let Some(h) = self.header {
            self.flush_one(&mut out, widths, &h);
        }
        for row in &self.rows {
            self.flush_one(&mut out, widths, row);
        }
    }

    fn flush_one(&self,
                 out: &mut impl std::io::Write,
                 widths: [usize; N],
                 row: &[(usize, usize, usize); N]) {
        let mut first = true;
        for i in 0..N {
            // Skip fully-empty columns
            if widths[i] == 0 {
                continue;
            }
            let (len, pos0, pos1) = row[i];
            if self.conf.out == OutStyle::Tab {
                if !first {
                    out.write_all(b"\t").unwrap_or(());
                }
                out.write_all(&self.buf[pos0..pos1]).unwrap_or(());
            } else {
                // Space between columns
                if !first {
                    out.write_all(self.margins[i].as_bytes()).unwrap_or(());
                }
                // Write the cell with alignment
                let pad = &SPACES[0..usize::min(SPACES.len(), widths[i] - len)];
                match self.aligns[i] {
                    Align::Right => {
                        out.write_all(pad).unwrap_or(());
                        out.write_all(&self.buf[pos0..pos1]).unwrap_or(());
                    },
                    Align::Left => {
                        out.write_all(&self.buf[pos0..pos1]).unwrap_or(());
                        if i < N - 1 {
                            out.write_all(pad).unwrap_or(());
                        }
                    },
                }
            }
            first = false;
        }
        out.write_all(self.conf.lineend).unwrap_or(());
    }

    #[cfg(test)]
    pub fn to_string(mut self) -> String {
        let mut out = Vec::with_capacity(self.buf.len());
        self.flush(&mut out);
        self.rows.clear();
        String::from_utf8(out).expect("Non-utf8 table output")
    }
}

impl<const N: usize> Drop for Table<'_, N> {
    /// Table is rendered to stdout when it goes out of scope
    fn drop(&mut self) {
        self.flush(BufWriter::new(stdout().lock()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parse::Ansi;

    #[test]
    fn last() {
        let conf = Conf::from_str("emlop log --color=n -H");

        // No limit
        let mut t = Table::<1>::new(&conf);
        for i in 1..10 {
            t.row([&[&format!("{i}")]]);
        }
        assert_eq!(t.to_string(), "1\n2\n3\n4\n5\n6\n7\n8\n9\n");

        // 5 max
        let mut t = Table::<1>::new(&conf).last(5);
        for i in 1..10 {
            t.row([&[&format!("{i}")]]);
        }
        assert_eq!(t.to_string(), "5\n6\n7\n8\n9\n");

        // 5 max ignoring header
        let mut t = Table::new(&conf).last(5);
        t.header(["h"]);
        for i in 1..10 {
            t.row([&[&format!("{i}")]]);
        }
        assert_eq!(t.to_string(), "h\n5\n6\n7\n8\n9\n");
    }

    #[test]
    fn align_cols() {
        let conf = Conf::from_str("emlop log --color=n --output=c");
        let mut t = Table::<2>::new(&conf).align_left(0);
        t.row([&[&"short"], &[&1]]);
        t.row([&[&"looooooooooooong"], &[&1]]);
        t.row([&[&"high"], &[&9999]]);
        let res = "short                1\n\
                   looooooooooooong     1\n\
                   high              9999\n";
        assert_eq!(t.to_string(), res);
    }

    #[test]
    fn align_longheader() {
        let conf = Conf::from_str("emlop log --color=n --output=c -H");
        let mut t = Table::<2>::new(&conf).align_left(0);
        t.header(["heeeeeeeader", "d"]);
        t.row([&[&"short"], &[&1]]);
        t.row([&[&"high"], &[&9999]]);
        let res = "heeeeeeeader     d\n\
                   short            1\n\
                   high          9999\n";
        assert_eq!(t.to_string(), res);
    }

    #[test]
    fn align_cols_last() {
        let conf = Conf::from_str("emlop log --color=n --output=c");
        let mut t = Table::<2>::new(&conf).align_left(0).last(1);
        t.row([&[&"looooooooooooong"], &[&1]]);
        t.row([&[&"short"], &[&1]]);
        let res = "short  1\n";
        assert_eq!(t.to_string(), res);
    }

    #[test]
    fn align_tab() {
        let conf = Conf::from_str("emlop log --color=n --output=t");
        let mut t = Table::<2>::new(&conf).align_left(0);
        t.row([&[&"short"], &[&1]]);
        t.row([&[&"looooooooooooong"], &[&1]]);
        t.row([&[&"high"], &[&9999]]);
        let res = "short\t1\n\
                   looooooooooooong\t1\n\
                   high\t9999\n";
        assert_eq!(t.to_string(), res);
    }

    #[test]
    fn color() {
        let conf = Conf::from_str("emlop log --color=y --output=c");
        let mut t = Table::<2>::new(&conf).align_left(0);
        t.row([&[&"123"], &[&1]]);
        t.row([&[&conf.merge, &1, &conf.dur, &2, &conf.cnt, &3, &conf.clr], &[&1]]);
        let res = "123  1\x1B[m\n\
                   \x1B[1;32m1\x1B[1;35m2\x1B[33m3\x1B[m  1\x1B[m\n";
        let (l1, l2) = res.split_once('\n').expect("two lines");
        assert_eq!(Ansi::strip(l1, 100), "123  1");
        assert_eq!(Ansi::strip(l1, 100), Ansi::strip(l2, 100));
        assert_eq!(t.to_string(), res);
    }

    #[test]
    fn nocolor() {
        let conf = Conf::from_str("emlop log --color=n --output=c");
        let mut t = Table::<2>::new(&conf).align_left(0);
        t.row([&[&"123"], &[&1]]);
        t.row([&[&conf.merge, &1, &conf.dur, &2, &conf.cnt, &3, &conf.clr], &[&1]]);
        let res = "123      1\n\
                   >>> 123  1\n";
        assert_eq!(t.to_string(), res);
    }
}
