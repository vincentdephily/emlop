use std::{fmt::Display,
          io::{stdout, Write as _}};

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
    rows: Vec<[(usize, usize, usize); N]>,
    /// Max column widths seen so far
    widths: [usize; N],
    /// Whether a column is fully empty and should be skipped
    empty: [bool; N],
    /// Line ending (may contain ansi cleanup chars)
    lineend: Vec<u8>,
    /// Column alignments (defaults to Right)
    aligns: [Align; N],
}

impl<const N: usize> Table<N> {
    /// Initialize new table
    pub fn new(lineend: &str) -> Table<N> {
        Self { rows: Vec::with_capacity(32),
               buf: Vec::with_capacity(1024),
               widths: [0; N],
               empty: [true; N],
               lineend: format!("{}\n", lineend).into(),
               aligns: [Align::Right; N] }
    }
    /// Specify column alignments
    pub fn align(mut self, col: usize, align: Align) -> Self {
        self.aligns[col] = align;
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
                write!(self.buf, "{}", s).expect("write to buf");
                //s.write(self.buf);
                if self.buf.get(p).map_or_else(|| false, |c| !c.is_ascii_control()) {
                    len += self.buf.len() - p;
                }
            }
            let pos2 = self.buf.len();
            self.widths[i] = usize::max(self.widths[i], len);
            self.empty[i] &= len == 0;
            idxrow[i] = (len, pos0, pos2);
        }
        self.rows.push(idxrow);
    }
}

impl<const N: usize> Drop for Table<N> {
    /// Table is rendered to stdout when it goes out of scope
    fn drop(&mut self) {
        let spaces = [b' '; 128];
        let stdout = stdout();
        let mut out = stdout.lock();
        for row in &self.rows {
            let mut first = true;
            for i in 0..N {
                // Skip fully-empty columns
                if self.empty[i] {
                    continue;
                }
                // Min space between columns
                if !first {
                    out.write_all("  ".as_bytes()).unwrap_or(());
                }
                first = false;
                // Write the cell with alignment
                let (len, pos0, pos1) = row[i];
                let pad = usize::min(spaces.len(), self.widths[i] - len);
                match self.aligns[i] {
                    Align::Right => {
                        out.write_all(&spaces[0..pad]).unwrap_or(());
                        out.write_all(&self.buf[pos0..pos1]).unwrap_or(());
                    },
                    Align::Left => {
                        out.write_all(&self.buf[pos0..pos1]).unwrap_or(());
                        if i < N - 1 {
                            out.write_all(&spaces[0..pad]).unwrap_or(());
                        }
                    },
                }
            }
            out.write_all(&self.lineend).unwrap_or(());
        }
    }
}