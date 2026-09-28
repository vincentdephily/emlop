#![cfg_attr(feature = "unstable", feature(test))]

//! This crate provides information about past and present Portage activity on a Gentoo system.
//!
//! # Reading historical data
//!
//! * [ELog] returns an iterator of [ELogLine]s, with filters for variant, timestamp, and
//!   content. Some post-analysis is needed, for example matching a `MergeStop` to a previous
//!   `MergeStart` (using their `key` field), to tell how long that merge took.
//! * [PkgMoves] returns the newest name of a renamed package.
//!
//! # Gathering info about ongoing merge
//! * [EmergeInfo] contains info about system processes, the emerge process tree, and the currently
//!   building packages.
//! * [Mtimedb] returns the current/backup merge list saved by portage, and info required by
//!   [PkgMoves].
//! * [get_pretend()] returns the merge list parsed from `emerge -pv`.
//! * [get_buildlog()] returns a one-line status from a package's `build.log`.
//!
//! # Misc utilities
//!
//! * [Times] accumulates durations and predicts the next one.
//! * [FmtUtc] formats a unix timestamp as ISO8601.
//! * [Ansi] strips ansi color sequences from logs.

// TODO: Make Arg{Parse,Error} optional, or even move it back to the binary ?

mod ansi;
mod current;
mod datetime;
mod history;
mod proces;
mod stats;
mod types;

pub use ansi::*;
pub use current::*;
pub use datetime::*;
pub use history::*;
pub use proces::*;
pub use stats::*;
pub use types::*;
