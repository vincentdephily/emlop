# 0.7.0 2024-03-04

Feature release: Multi-term search, config file, and many goodies

## New features

* Support searching by multiple terms
  - eg `emlop s -e gcc clang llvm rust`
* Support configuration file
  - Located at `~/.config/emlop.toml`, or whereever `$EMLOP_CONFIG` says
  - Example file added to repo, should be installed alongside emlop docs
  - All available config options correspond to a cli arg, which takes precedance
* Support reading gzip-compressed `emerge.log.gz` file
* Autodetect `tmpdir` using currently running emerge processes
* Assume unknown packages take 10s (overridable with `--unknown`) to compile instead of 0

## Interface changes

* Support multiple `--tmpdir` arguments
* `--tabs` has been renamed `--output=tab/columns/auto` (or `-ot` for short)
  - Default output is now `columns` on tty and `tab` otherwise, to simplify `emlop ...|cut -f...` workflow
* Added `--resume=either` variant to resume either main or backup list
  - Passing `--resume` without argument is now the same as `--resume either`
  - Default value remains `auto` (use main list if currently emerging)
* `--color` variants renamed to `(y)es`, `(n)o`, `(t)ty`
* Improved inline help and error messages
  - Upgraded argument parser dependency
  - We now do our own parsing and error rendering for many args
  - Color scheme, style, and texts updated

## Bug fixes and misc

* Fixed column width when `--last` is used
* Fixed noticing a failed/interruped merge when the emerge proces keeps going
* Ignore a performance-sensitive test during emerge
* Display elapsed time also while compiling an unknown package
* Raised MSRV to 1.71

# 0.6.1 2023-06-23

Maintenance release.

* Refactored tests to avoid deadlock when testing via portage
  - As a bonus, tests finish much faster
* Reduced packaged crate size by compressing image and removing unwanted files
* Routine deps update


# 0.6.0 2023-06-20

Feature release: Improved `predict` command and accuracy, filter sync events, new output options,
and more.

## Main changes

* `predict` command now reads portage's merge resume list
  - `emlop p` behaves the same as `emerge -rOp|emlop p` if a merge is currently running
  - Including geting confused when you have multiple emerge commands running
  - You can still pipe any `emerge -p` command into `emlop p`
  - See `--resume` to tweak the behavior
* `predict` command now displays the current build phase and last log line of ongoing builds
  - This usually requires root permissions
* `list/stat` search now also applies to sync repos, eg `emlop l -ss guru`
* Improved `stats` layout
  - Group by section (syncs/packages/total) before grouping by date
  - Each section has its specific headers and colum count
  - Display at most 3 sections (tables), separated by a newline
* Added `--show` option to `predict`, like in other subcommands
* Added `--first/-N/--last/-n` options to limit output
* Added `--tabs` option for more machine-readable output
* Added `--avg=mean/median/weighted` option to tweak predictions
  - `mean` is the original behaviour
  - `median` is the new default, it's more resilient against noisy data
  - `weighted-{arith,median}` gives more importance to recent values
* Added `--starttime` option to show merge start instead of merge end
* Moved `--show h` option value to a dedicated `--headers`/`-H` flag
  - Fixes surprising behavior when only `h` was specified
  - More discoverable
* Added `accuracy` subcommand to evaluate prediction accuracy
  - Mostly a development tool, but you might find it useful

## And also

* Long args like `--duration` can now be abbreviated
* `--color` is now an alias of `--color=always`
* Bash completion no longer wrongly suggests `-V`/`--version` for subcommands
* Exit code on error changed to `1` for empty results and `2` for plain errors
* Various performance improvements
  - Now clearly faster than `qlop` in all cases
* Improved and reorganized inline help
* Use asciinema recordings in readme
* Internal stuff
  - Increased MSRV to 1.65
  - Updated github CI actions
  - Upgraded `clap` (triggered a lot of this release's improvements)
  - Recover from process listing errors
  - Various refactorings
  - Removed unmaintained `ansi_term` and `sysconfig`
  - New/improved benchmark tools

# 0.5.0 2022-04-18

Feature release: date and duration parsing/formating, per-repo sync stats

* Added a `--utc` flag to use that timezone when displaying or parsing (command-line) dates
* Added a `--date` argument to change date output format
  - The default format no longer includes the utc offset
* New duration output formats
  - `hms_fixed` (eg "00:01:23", thanks to Flexibeast)
  - `human` (eg "1 munute, 23 seconds")
* Added optional column headers for `log` and `stats` (part of `--show` argument)
* Sync events in `log` and `stat` commands now mention the synced repo
  - The stats make sense again, on systems with multiple portage repos
* Simplified the format of `--from`/`--to` arguments
  - For example, you can write `1w` or `1 week` instead  of `1 week ago`
  - Some wordyer variants are no longer supported

There are also a number of significant internal changes and refactorings:

* Ported away from the `chrono` crate
  - It's lacking maintenance and suffering from unsoundness
  - One caveat is that the new crate (`time`) only supports getting the current UTC offset, meaning
    that DST changes are no longer reported in emlop's output (the datetime is still correct, but
    the offset may not be correct for that date). This will hopefully be supported at some stage.
* New table alignment engine
  - Left-aligned columns are now only as wide as necessary
  - Marginally faster than the generic crate
  - Easier to add features (headers, csv, sorting...)
* Other internal changes
  - More robust unittests
  - Dep updates including some audit fixes
  - Refactorings

# 0.4.2 2021-08-19

Maintenance release

* Fix some unittest failures when run from portage or unexpected timezone
* Updated deps
  - Routine updates
  - Closes RUSTSEC-2020-0095 advisory (likely no vulnerability)
  - Raises MSRV to 1.52 (current oldest version in Gentoo)
* Various refactorings
  - Readability
  - Clippy lints
  - Changed deps API
* Doc updates

# 0.4.1 2020-12-26

Maintenance release

* Fix rare panic when printing system process commandline.
  Previous fix didn't quite do it, this one has an associated test.
* Switch to panic=abort (2% speedup, smaller binary)
* Routine deps update

# 0.4.0 2020-10-19

Feature release: unmerge events, shell completion, predicted merge timestamp, and optimizations.

Long time no release, gotta get those goodies out of the door, perhaps even in the portage tree :)

* Display predicted merge timestamp (not just duration).
* Display unmerge logs (optional: `--show u`) and stats (always).
  `stats --show m` renamed to `stats --show p`
* Stats columns were reordered to be more consistent.
* Stats give the predicted rather than average time where it makes sense.
* Added shell completion scripts for bash, zsh, fish.
* `-f/-t` are now aliases of `--from/--to`.
  `-f` as an alias of `--logfile` was renamed to `-F`.
* Various optimisations, emlop is now unambiguously faster than its peers.
* Fix handling of negative durations (system clock gone backwards).
  Also, if warnings are enabled clock jumps will be reported.
* Fix rare panic when printing system process comandline.
* Code cleanups, dep updates, expanded unittests, bumped MSRV to 1.41.1.

# 0.3.1 2019-06-10

Maintenance release: Fix a panic and do some code maintenance.

Not much development since the bugfix in march, so might as well release it now :) I've been busy
with other things this year, but more emlop feature releases are comming.

* Fixed potential panic when grouping stats by month.
* Switched to rust2018 (raises minimum rust version to 1.31).
* Switched to a faster split function (raises minimum rust version to 1.34).
* Minor code cleanups.
* Deps refresh; the compiled binary is a bit smaller.

# 0.3.0 2018-12-24

Feature release: Sync support, improved stats, and optimisations.

The two highlights are support for displaying Sync events, and support for grouping stats by
timespan. Other niceties include a `Total` stats row, displaying durations in seconds, and threaded
parsing. Happy Christams to those who celebraye it :)

* Renamed `list` subcommand to `log` (kept `list` as a hidden alias).
* Added option to show `Sync` in log and stat commands.
* Added option to show `Total` in stat command.
* Added option to group stats by year/month/week/day.
* Added option to format durations in seconds instead of hours:minutes:seconds.
* Various CLI and online help improvements.
* Now using a separate thread for parsing, speeding things up a bit.
* Bumped minimum rust version to 1.30.
* Code is now formated using rustfmt.
* A bunch of of small bugfixes, QA fixes, and optimisations.

# 0.2.0 2018-04-28

Feature release: Filtering, color, speed, and lots of polish.

Improved filtering, added colors, sped up everything, fixed bugs, and fixed interface papercuts (see
details below). This release fixes many first-impression annoyances of the first, so give emlop
another try if you haven't switched yet ;)

* All output is now colored (controlable via `--color` flag).
* Regexp search is now case-insensitive: no need to remember the casing of 'PyQt' anymore.
* New `--exact`/`-e` (non-regexp) search mode (like the default/only mode of `{gen,q,pq,go}lop`).
* New `--from`/`--to` arguments to filter by date.
* Predict now displays ebuild versions.
* Big speedup: `emlop` is about twice as fast, now on par with `qlop`.
* New `-v` flag to specify log level error/warning/info/debug.
* Warnings are now hidden by default, while new info/debug messages have been added.
* Improved inline help and fixed some argument-passing papercuts.
* Fixed bugs #6, #7, 8 and some unreported ones.
* Shell exit codes are now well-defined and documented.
* Various refactorings, unittest improvements, and general polish.

# 0.1 2018-03-07

First release, huzza !

The core commands `list`, `predict`, and `stats` are here and emlop already feels like a worthy
replacement for genlop (by being faster) or qlop (by having a predict mode and regexp search).

Thanks to singul0 for fixing a panic when outputing to a closed pipe.
