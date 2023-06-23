# Emlop roadmap
Here are some things that I'd like to eventually do for emlop. In no particular order, some that I
really want, some no more than an idle tought. If any of those resonate with you, please
[contribute](CONTRIBUTING.md) with an issue report or a pull request.

## Refactoring
### Re-investigate using clap's derive API, or a different crate like bpaf
### Better parallelization
Currently a 'parsing' and a 'main' tread but should be possible to parse using all cores.
* https://github.com/alex-shapiro/ditto might help with ordering ?
* StreamExt.buffered_unordered
* A pure IO thread ?
* Switch to proper async ?

## Features
### Use colors to carry mmore information
* Color-code predict durations ?
* Dark-green for packages not in world file ?
### Parse and optionaly display failed merges
### Distinguish autoclean from explicit unmerges
### Show previous version for upgrades
### Json output
Because why not. Could help with unit-testing.
### Extra info in stats command
* use flags and build env of current install (like genlop)
* date of first/last merge
* build time variability
* build time trend
* distinct version count
### Sort stats by any column
### Config file to set defaults
### Allow specifying multiple search strings
### embed audit info in binary
https://github.com/Shnatsel/rust-audit
### stats visuals
* https://docs.rs/rspark/ or https://docs.rs/spark/
### TUI for `predict`
* Run in a loop until merge finishes
* Limit to one screenfull (elide the middle lines)
### Generate manpage
### Selectable and sortable output columns
