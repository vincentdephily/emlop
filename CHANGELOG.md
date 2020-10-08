# Unreleased

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
* Code cleanups, dep updates, expanded unittests, bumped MSRV to 1.37.0.

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
