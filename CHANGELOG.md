# Unreleased

* Renamed `list` subcommand to `log` (kept `list` as a hidden alias).
* Added option to show `Sync` in log and stat commands.
* Added option to show `Total` in stat command.
* Added option to group stats by year/month/week/day.
* Added option to format durations in seconds instead of hours:minutes:seconds.
* Various CLI and online help improvements.
* Now using a separate thread for parsing, speeding things up a bit.
* Bumped minimum rust version to 1.30.
* A bunch of of small bugfixes, QA fixes, and optimisations.

# 0.2.0 2018-04-28

Second release: Filtering, color, speed, and lots of polish.

Improved filtering, added colors, sped up everything, fixed bugs, and
fixed interface papercuts (see details below). This release fixes many
first-impression annoyances of the first, so give emlop another try if
you haven't switched yet ;)

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

The core commands `list`, `predict`, ans `stats` are here and emlop already feels like a worthy
replacement for genlop (by being faster) or qlop (by having a predict mode and regexp search).

Thanks to singul0 for fixing a panic when outputing to a closed pipe.
