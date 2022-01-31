# Emlop roadmap
Here are some things that I'd like to eventually do for emlop. In no particular order, some that I
really want, some no more than an idle tought. If any of those resonate with you, please
[contribute](CONTRIBUTING.md) with an issue report or a pull request.

## Testing
### Misc
* More testcases
### https://github.com/japaric/trust/

## Refactoring
### StructOpt crate
There's a `structopt` branch doing just that, but the end result is not as convincing as I hoped.
### http://casualhacks.net/blog/2018-03-10/exploring-function-overloading/
For nicer parser implementation ?
### Better parallelization
Currently a 'parsing' and a 'main' tread but should be possible to parse using all cores.
* https://github.com/alex-shapiro/ditto might help with ordering ?
* StreamExt.buffered_unordered
* Use something more low-level than `.lines()` ?
* A pure IO thread ?
* Swithc to proper async ?
### Reduce allocations during parsing
Should be able to take slices from the input instead of allocating new Strings.
https://deterministic.space/secret-life-of-cows.html ?
https://docs.rs/regex/1.4.3/regex/struct.Regex.html#method.captures_read_at
### Low-level optims
Use `flame` and `flamer` crates ?
### Remove utf8 validation overhead
https://github.com/BurntSushi/bstr

## Features
### Optional --headers
### Use colors to carry mmore information
* Color-code predict durations ?
* Dark-green for packages not in world file ?
### Automatically run `emerge -rOp` for `predict`
### Parse and optionaly display failed merges
### Distinguish autoclean from explicit unmerges
### Show previous version for upgrades
### Get ebuild upstreamed
https://bugs.gentoo.org/649904
### Option to limit display to last/first N events
### Better/selectable prediction algorythm
Currently is just averaged over the last `--limit` times, but should probably be a weighted average.

One of the weight is obviously how recent a particular build is. The current algorythm is equivalent
to giving a weight of 1 to all builds within `--limit` and 0 to all others, but using a `log()`
function to assign weigths might be better.

Another weigth should probably be how close slot or the version is (for example, qtsvg:4 takes
longer to compile than qtsvg:5, but the older version is still regularly compiled on my system).

Ignore outlyers (abnormally long merges), maybe using the mean might be better than the average.
### Json output
Because why not. Could help with unit-testing.
### Pull timings from gentoo.linuxhowtos.org for first-time emerge
Never used this in genlop, but I guess others will want the feature.
### Extra info in stats command
* use flags and build env of current install (like genlop)
* date of first/last merge
* build time variability
* build time trend
* distinct version count
### Sort stats by any column
### Config file to set defaults
### Bash completion
Clap has something builtin.
### Allow specifying multiple search strings
### embed audit info in binary
https://github.com/Shnatsel/rust-audit
### stats visuals
* https://docs.rs/rspark/ or https://docs.rs/spark/
