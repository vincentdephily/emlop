use crate::Styles;
use std::{collections::VecDeque,
          fmt::Display,
          io::{stdout, BufWriter, Write as _}};

#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Right,
}

pub struct Table<const N: usize> {
    /// Buffer where unaligned entries are written
    ///
    /// We can only render alignments when we saw all the rows.
    /// Having a single buffer noticable speed things up by reducing allocations.
    buf: Vec<u8>,
    /// Visible length, and start/stop index into buffer
    rows: VecDeque<[(usize, usize, usize); N]>,
    /// Max column widths seen so far
    widths: [usize; N],
    /// Whether a header has been set
    have_header: bool,

    /// Line ending (may contain ansi cleanup chars)
    lineend: Vec<u8>,
    /// Column alignments (defaults to Right)
    aligns: [Align; N],
    /// Margin between columns, printed left of the column, defaults to `"  "`
    margins: [&'static str; N],
    /// Only print last N rows
    last: usize,
    /// Align using tabs
    tabs: bool,
}

impl<const N: usize> Table<N> {
    /// Initialize new table
    pub fn new(st: &Styles) -> Table<N> {
        Self { rows: VecDeque::with_capacity(32),
               buf: Vec::with_capacity(1024),
               widths: [0; N],
               have_header: false,
               lineend: format!("{}\n", st.clr).into(),
               aligns: [Align::Right; N],
               margins: ["  "; N],
               last: usize::MAX,
               tabs: st.tabs }
    }
    /// Specify column alignments
    pub fn align(mut self, col: usize, align: Align) -> Self {
        self.aligns[col] = align;
        self
    }
    /// Specify column left margin (1st printted column never has a left margin)
    pub fn margin(mut self, col: usize, margin: &'static str) -> Self {
        self.margins[col] = margin;
        self
    }
    /// Specify column left margin (1st printted column never has a left margin)
    pub fn last(mut self, last: usize) -> Self {
        self.last = last;
        self
    }
    /// Add a section header
    pub fn header(&mut self, enabled: bool, row: [&str; N]) {
        if enabled {
            if !self.rows.is_empty() {
                self.row([&[]; N]);
            }
            self.last = self.last.saturating_add(1);
            self.have_header = true;

            let mut idxrow = [(0, 0, 0); N];
            for i in 0..N {
                let start = self.buf.len();
                self.buf.extend(row[i].as_bytes());
                self.widths[i] = usize::max(self.widths[i], row[i].len());
                idxrow[i] = (row[i].len(), start, self.buf.len());
            }
            self.rows.push_back(idxrow);
        }
    }
    /// Add one row of data
    ///
    /// The number of cells is set by const generic.
    /// Each cell is an array of displayables.
    /// Entries that start with an ascii control char are assumed to be zero-length.
    pub fn row(&mut self, row: [&[&dyn Display]; N]) {
        let mut idxrow = [(0, 0, 0); N];
        for i in 0..N {
            let start = self.buf.len();
            let mut len = 0;
            for s in row[i] {
                let p = self.buf.len();
                write!(self.buf, "{s}").expect("write to buf");
                if self.buf.get(p).map_or_else(|| false, |c| !c.is_ascii_control()) {
                    len += self.buf.len() - p;
                }
            }
            self.widths[i] = usize::max(self.widths[i], len);
            idxrow[i] = (len, start, self.buf.len());
        }
        self.rows.push_back(idxrow);
        if self.rows.len() > self.last {
            if self.have_header {
                self.rows.swap(0, 1);
            }
            self.rows.pop_front();
        }
    }

    fn flush(&self, mut out: impl std::io::Write) {
        let spaces = [b' '; 128];
        for row in &self.rows {
            let mut first = true;
            // Clippy suggests `for (i, <item>) in row.iter().enumerate().take(N)` which IMHO
            // doesn't make sense here.
            #[allow(clippy::needless_range_loop)]
            for i in 0..N {
                // Skip fully-empty columns
                if self.widths[i] == 0 {
                    continue;
                }
                let (len, pos0, pos1) = row[i];
                if self.tabs {
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
                    let pad = &spaces[0..usize::min(spaces.len(), self.widths[i] - len)];
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
            out.write_all(&self.lineend).unwrap_or(());
        }
    }
}

impl<const N: usize> Drop for Table<N> {
    /// Table is rendered to stdout when it goes out of scope
    fn drop(&mut self) {
        self.flush(BufWriter::new(stdout().lock()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn check<const N: usize>(mut tbl: Table<N>, expect: &str) {
        let mut out = Vec::with_capacity(tbl.buf.len());
        tbl.flush(&mut out);
        tbl.rows.clear();
        assert_eq!(expect, String::from_utf8(out).unwrap());
    }

    #[test]
    fn last() {
        let st = Styles::from_str("emlop log --color=n");

        // No limit
        let mut t = Table::<1>::new(&st);
        for i in 1..10 {
            t.row([&[&format!("{i}")]]);
        }
        check(t, "1\n2\n3\n4\n5\n6\n7\n8\n9\n");

        // 5 max
        let mut t = Table::<1>::new(&st).last(5);
        for i in 1..10 {
            t.row([&[&format!("{i}")]]);
        }
        check(t, "5\n6\n7\n8\n9\n");

        // 5 max ignoring header
        let mut t = Table::new(&st).last(5);
        t.header(true, ["h"]);
        for i in 1..10 {
            t.row([&[&format!("{i}")]]);
        }
        check(t, "h\n5\n6\n7\n8\n9\n");
    }
}
