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
    /// Whether a column is fully empty and should be skipped
    empty: [bool; N],
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
               empty: [true; N],
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
    pub fn header(&mut self, enabled: bool, row: [&[&dyn Display]; N]) {
        if enabled {
            if !self.rows.is_empty() {
                self.row([&[]; N]);
            }
            self.row(row);
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
            let pos0 = self.buf.len();
            let mut len = 0;
            for s in row[i] {
                let p = self.buf.len();
                write!(self.buf, "{s}").expect("write to buf");
                if self.buf.get(p).map_or_else(|| false, |c| !c.is_ascii_control()) {
                    len += self.buf.len() - p;
                }
            }
            let pos2 = self.buf.len();
            if !self.tabs {
                self.widths[i] = usize::max(self.widths[i], len);
            }
            self.empty[i] &= len == 0;
            idxrow[i] = (len, pos0, pos2);
        }
        self.rows.push_back(idxrow);
        if self.rows.len() > self.last {
            self.rows.pop_front();
        }
    }
}

impl<const N: usize> Drop for Table<N> {
    /// Table is rendered to stdout when it goes out of scope
    fn drop(&mut self) {
        let spaces = [b' '; 128];
        let mut out = BufWriter::new(stdout().lock());
        for row in &self.rows {
            let mut first = true;
            // Clippy suggests `for (i, <item>) in row.iter().enumerate().take(N)` which IMHO
            // doesn't make sense here.
            #[allow(clippy::needless_range_loop)]
            for i in 0..N {
                // Skip fully-empty columns
                if self.empty[i] {
                    continue;
                }
                let (len, pos0, pos1) = row[i];
                if self.tabs {
                    if !first {
                        out.write_all(b"\t").unwrap_or(());
                    }
                    out.write_all(&self.buf[pos0..pos1]).unwrap_or(());
                } else {
                    // Min space between columns
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
