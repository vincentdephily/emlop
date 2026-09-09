mod ansi;
mod proces;

pub use ansi::{AnsiStr, Theme};
pub use proces::FmtProc;
#[cfg(test)]
pub use proces::tests::procs;
