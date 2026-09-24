#![cfg_attr(feature = "unstable", feature(test))]

//! This crate provides information about past and present Portage activity on a Gentoo system.
//!
//! # Reading historical data
//!
//! * [get_hist()] returns an iterator of [HistEvent]s, with filters for variant, timestamp, and
//!   content. Some post-analysis is needed, for example matching a `MergeStop` to a previous
//!   `MergeStart` (using their `key` field), to tell how long that merge took.
//! * [PkgMoves.get()](PkgMoves::get()) returns the newest name of a renamed package.
//!
//! # Gathering info about ongoing merge
//! * [EmergeInfo] contains info about system processes, the emerge process tree, and the currently
//!   building packages.
//! * [Mtimedb.resume_list()](Mtimedb::resume_list()) returns the current/backup merge list saved by portage.
//! * [get_pretend()] returns the merge list parsed from `emerge -pv`.
//! * [get_buildlog()] returns a one-line status from a package's `build.log`.
//!
//! # Misc utilities
//!
//! * [Times] accumulates durations and predicts the next one.
//! * [FmtUtc] formats a unix timestamp as ISO8601.
//! * [Ansi] strips ansi color sequences from logs.

// TODO: Show::{tot,pkg} don't belong in the library, split this type
// TODO: Make Arg{Parse,Error} optional, or even move it back to the binary ?
// TODO: Make serde (Mtimedb,PkgMove,get_resume...) optional ?

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
