# Comparison with other emerge log parsers

Original motivation for Emlop was a faster/more accurate version of `genlop -p`, and learning
Rust. It has since gained features and maturity to compete on all fronts. This file compares
`genlop-0.30.11`, `qlop-0.96.1`, and `emlop-0.7.0`. Please report any outdated/incorrect info using
the [issue tracker](https://github.com/vincentdephily/emlop/issues).

Known emerge log parsers:
* [Emlop](https://github.com/vincentdephily/emlop) (Rust) is the one you're reading about.
* [Genlop](https://github.com/gentoo-perl/genlop) (Perl) is the most well known.
* [Qlop](https://github.com/gentoo/portage-utils) (C) is pretty fast and part of a larger toolkit.
* [Splat](http://www.l8nite.net/projects/splat/) (Perl) looks like Genlop's predecessor, dead upstream.
* [Pqlop](https://bitbucket.org/LK4D4/pqlop) (Python) was an ambitious rewrite, dead upstream.
* [Glop](https://github.com/kongo2002/glop) (Haskell) was a simple rewrite, dead upstream.
* [Golop](https://github.com/klausman/golop) (Go) is a recent rewrite apparently abandoned quickly.
* [Emwa](https://github.com/foxtrot-wx/emwa) (C) is a recent addition, time will tell.

Rust, Perl, C, Python, Haskell, Go... at least Gentoo doesn't suffer from a language monoculture ;)

## Interface

Emlop is organised into subcommands, whereas {gen,q}lop only use (possibly conflicting) flags. It
tries to merge functions where that makes sense, for example `emlop l` combines `genlop -l`, `genlop
-e`, and `genlop -t`, because there didn't seem to be a point to separate them. Same thing with
`genlop -c` and `genlop -p` which are combined into `emlop p`.

## Output

Emlop output aims to be compact, beautiful, flexible, and easy to read/parse. Qlop is similar to
genlop, but did make some outputs more compact.

Default qlop duration output depends on length: `45s` -> `3′45″` -> `1:23:45`. Machine output
applies to dates and durations at the same time.

|                              | genlop | qlop          | emlop   |
| :------------------------    | :----: | :-----------: | :-----: |
| Output density               | sparse | compact       | compact |
| Optional headers             | no     | no            | yes     |
| Aligned output               | some   | some          | all     |
| Optional plain tab alignment | no     | no            | yes     |
| Force color output           | no     | yes           | yes     |
| Date output formats          | -      | rfc3339,ts    | many    |
| Timezone options             | utc    | -             | utc     |
| Duration output formats      | text   | hms,secs,text | many    |

## Merge log

|                                                       | genlop   | qlop   | emlop    |
| :---------------------------------------------------- | :----:   | :----: | :------: |
| Display merges/unmerges                               | yes      | yes    | yes      |
| Distinguish autoclean/manual unmerges                 | no       | yes    | no       |
| Distinguish syncs per repository                      | no       | no     | yes      |
| Display unmerge/sync duration                         | no       | yes    | yes      |
| Display interrupted/failed merges                     | no       | no     | no       |
| Display currently installed package's USE/CFLAGS/date | yes      | no     | no       |
| Display merge begin time or end time by default       | end only | begin  | end      |

If the log contains a merge end event without a merge start, qlop displays nothing, genlop displays
a buggy time, and emlop displays the time as `?`. Qlop also displays nothing when time jumps
backward. As a result, qlop may report fewer total merges than {gen,em}lop.

Qlop sync duration only corresponds to the first repo (typically `gentoo`). Emlop sync duration
ignores the pre-sync setup time (usually 0 or 1 seconds).

## Merge stats

Emlop has a dedicated `stats` command. {gen,q}lop spread the functionality between multiple and
sometimes incompatible flags.

|                                                          | genlop | qlop  | emlop |
| :------------------------------------------------------- | :----: | :---: | :---: |
| Individual merge count/total/average/prediction          | c,t,a  | c,t,a | c,t,p |
| Total merge count/total/average                          | -      | c,t,a | c,t,a |
| Total unmerge count/total/average                        | -      | c,t,a | c,t,a |
| Total sync count/total/average/prediction                | -      | c,t,a | c,t,p |
| Group stats by year/month/week/day                       | no     | no    | yes   |

## Filtering

Genlop switches case-sensitivity using `-s` vs `-S` flag. Emlop doesn't have a flag, but regexp can
be prepended with `(?-i)` should case-sensitivity ever be needed. Qlop only supports plaintext
whole-word matching.

Genlop and qlop use a single flag for min/max date, so it isn't possible to specify only a max
date.

For relative dates, genlop accepts fancy strings like "last month" or "2 weeks ago", qlop is a bit
less flexible but less verbose (no "ago" needed), and emlop only accepts a number of days/weeks/etc
which can be abbreviated (for example "1 week, 3 days" -> "1w3d").

|                                          | genlop      | qlop  | emlop       |
|:-----------------------------------------|:-----------:|:-----:|:-----------:|
| Limit log parsing by date                | yes         | yes   | yes         |
| Limit log to number fisrt/last n entries | no          | no    | yes         |
| Limit log to last emerge operation       | no          | yes   | no          |
| Filter by package categ/name             | yes         | yes   | yes         |
| Filter by sync repo                      | no          | no    | yes         |
| Read filter list from file               | no          | yes   | no          |
| Search modes                             | plain/regex | plain | plain/regex |
| Default search mode                      | plain       | plain | regex       |

## Merge time prediction

Genlop uses the mean of the last 10 builds, ignoring the worst/best times. Qlop uses the mean of the
last 20 builds. Emlop uses the median of the last 15 builds, with options for other window sizes and
other averages (median/mean/weighted). Using a window mitigates against evolving build times, using
a median mitigates against exceptional build times. The Emlop defaults have been measured to give
significantly better accuracy over a full emerge log.

Qlop can only predict the current merge. Genlop and Emlop can also predict pretended merges (the
output of `emerge -p foo`). Emlop by default predicts the current full merge list (similar to what
piping `emerge -rOp` would do).

Genlop has multiple estimation bugs where data get mixed up (different categories, parallel merges,
etc). `Genlop -p` doesn't take current elapsed emerge time into account. When run as a normal user,
qlop warns about missing /proc permissions, finds bogus current merges, and doesn't give the same
ETA for the ones it finds. The linuxhowtos db is unmaintained and unlikely to contain info for your
CPU and ebuilds.

All tools give pessimistic prediction when packages are merged in parallel, because they assume
sequential merging. Even if they detected an ongoing parallel merge, it's not clear how they would
estimate the resulting speedup factor.

|                                                    | genlop        | qlop          | emlop                |
| :------------------------------------------------- | :----:        | :--:          | :---:                |
| Show ongoing merge ETA                             | current build | current build | whole list           |
| Show `emerge -p` merges ETA                        | yes           | no            | yes                  |
| Show individual merge ETAs                         | no            | no            | yes                  |
| Show current merge stage                           | no            | no            | yes                  |
| Global ETA format                                  | total time    | total time    | total time, end date |
| Estimation accuracy                                | ok            | better        | best, configurable   |
| Query gentoo.linuxhowtos.org for unknown packages  | yes           | no            | no                   |

## Speed

Here are timings for some common commands (in milliseconds, 95th centile of 50 runs, see
`benches/stdcomp.sh`) on a Ryzen 7 4700U with an SSD and the `benches/emerge.log` file with ~10K
merges.

The commands were selected to be comparable, but Some differences do influence timings. Emlop always
show merge time and package version in "log" mode, and looks up portage resume data in "predict"
mode. Genlop can't show unmerges of specific package only. Qlop -r still searches the log for
unfinished merges when it doesn't find an ongoing merge. Filtering by plaintext isn't noticeably
faster than by case-(in)sensitive regexp ({gen,em}lop only).

|                                                            | genlop | qlop | emlop |
|:-----------------------------------------------------------|-------:|-----:|------:|
| `genlop -l; qlop -m; emlop l`                              |    701 |   85 |    61 |
| `genlop -lut; qlop -muUvt; emlop l -smu`                   |    931 |  140 |   109 |
| `genlop gcc; qlop -m gcc; emlop l -e gcc`                  |    648 |   34 |    19 |
| `genlop -r --date 2020-10-08; qlop -stl; emlop l -ss -n`   |    625 |   65 |    12 |
| `emerge dummybuild&;genlop -c;qlop -r;emlop p`             |    760 |   77 |    70 |
| `genlop -p < emerge-p.gcc.out; emlop p < emerge-p.gcc.out` |    669 |  n/a |    46 |
| `genlop -p < emerge-p.qt.out;  emlop p < emerge-p.qt.out`  |   3383 |  n/a |    48 |
| `genlop -p < emerge-p.kde.out; emlop p < emerge-p.kde.out` |  20063 |  n/a |    46 |

Emlop is faster than qlop, which is already comfortably fast (the wall time is often dominated by
the terminal emulator). Genlop is noticably slow for basic tasks, and can be prohibitively slow for
`emerge -p` ETAs.

## misc

|                            | genlop       | qlop   | emlop             |
|:---------------------------|:------------:|:------:|:-----------------:|
| Shell completion           | bash         | none   | bash/zsh/fish/... |
| Complete package name      | yes          | n/a    | no                |
| Configuration file         | no           | no     | yes               |
| Read compressed emerge.log | yes          | no     | yes               |
| Unittests                  | no           | yes    | yes               |
| Documentation and help     | ok           | good   | good              |
| Development activity       | unmaintained | active | active            |
