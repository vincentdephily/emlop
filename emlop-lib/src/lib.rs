#![cfg_attr(feature = "unstable", feature(test))]

//! This crate provides information about past and present emerge activity on a Gentoo system
//!
//! # Reading historical data
//!
//! * [get_hist()] an iterator of filtered [HistEvent]s. Some post-analysis is needed, for
//!   example matching a `MergeStop` to a previous `MergeStart` (using their `key` field), to tell how
//!   long that merge took.
//! * [PkgMoves] (initialized using [Mtimedb]) keeps track of package renames.
//!
//! # Gathering info about ongoing merge
//! * The fields of [EmergeInfo] contains info about live emerge processes
//! * [get_resume()]  returns the merge list according to [Mtimedb]
//! * [get_pretend()]  returns the merge list according to `emerge -pv`
//!
//! # Misc utilities
//!
//! * [FmtUtc] To format a unix timestamp as ISO8601
//! * [Ansi] To strip ansi color sequences from logs

// TODO: Move commands::{Times,Stats} into library
// TODO: Show::{tot,pkg} don't belong in the library, split this type
// TODO: Make Arg{Parse,Error} optional, or even move it back to the binary ?
// TODO: Make serde (Mtimedb,PkgMove,get_resume...) optional ?

mod ansi;
mod current;
mod datetime;
mod history;
mod proces;
mod types;

pub use ansi::*;
pub use current::*;
pub use datetime::*;
pub use history::*;
pub use proces::*;
pub use types::*;
