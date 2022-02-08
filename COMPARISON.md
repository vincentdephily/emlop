# Comparison with other emerge log parsers

Original motivation for Emlop was a faster/more accurate version of `genlop -p`, and learning
Rust. It has since gained features and maturity to compete on all fronts. This file compares
`genlop-0.30.10`, `qlop-0.90`, and `emlop-0.4.0`. Please report any outdated/incorrect info using
[the issue tracker](https://github.com/vincentdephily/emlop/issues).

Known emegre log parsers:
* [Emlop](https://github.com/vincentdephily/emlop) (Rust) is the one you're reading about.
* [Genlop](https://github.com/gentoo-perl/genlop) (Perl) is the most well known and the inspiration
  for Emlop.
* [Qlop](https://github.com/gentoo/portage-utils) (C) is pretty fast and part of a larger toolkit.
* [Splat](http://www.l8nite.net/projects/splat/) (Perl) look like Genlop's predecessor, dead upstream.
* [Pqlop](https://bitbucket.org/LK4D4/pqlop) (Python) was an ambitious rewrite, dead upstream.
* [Golop](https://github.com/klausman/golop) (Go) is a recent rewrite apparently abandoned quickly.

Perl, Python, C, Go, Rust... at least Gentoo doesn't suffer from a language monoculture ;)


## Interface

Emlop is organised into subcommands, whereas {gen,q}lop only use (possibly conflicting) flags. It
tries to merge functions where that makes sense, for example `emlop l` combines `genlop -l`, `genlop
-e`, and `genlop -t`, because there didn't seem to be a point to separate them. Same thing with
`genlop -c` and `genlop -p` which are combined into `emlop p`.

## Output

Emlop output aims to be compact, beautiful, and easy to read/parse. Qlop is similar to genlop, but
did make some outputs more compact.

Default qlop duration output depends on length: `45s` -> `3′45″` -> `1:23:45`. Machine output
applies to dates and durations at the same time.

|                                               | genlop | qlop       | emlop           |
|:----------------------------------------------|:------:|:----------:|:---------------:|
| Output density                                | sparse | medium     | compact         |
| Colorized output                              | yes    | yes        | yes             |
| Date output options                           | utc    | iso,unix   | -               |
| Duration output style (seconds,hh:mm:ss,text) | text   | hms,s,text | hms,hms_fixed,s |
| Aligned output                                | some   | some       | all             |
| Headers                                       | no     | no         | no              |

## Merge log

|                                                       | genlop | qlop  | emlop |
| :---------------------------------------------------- | :----: | :---: | :---: |
| Display merges                                        | yes    | yes   | yes   |
| Display syncs                                         | yes    | yes   | yes   |
| Display unmerges                                      | yes    | yes   | yes   |
| Distinguish autoclean/manual unmerges                 | no     | yes   | no    |
| Display unmerge/sync time                             | no     | yes   | yes   |
| Display interrupted merges                            | no     | no    | no    |
| Display currently installed package's USE/CFLAGS/date | yes    | no    | no    |
| Display merge begin time or end time                  | end    | begin | end   |

If the log file is truncated and contains a merge end event without a merge start : qlop displays
nothing, genlop displays a buggy time, emlop displays the time as `?`.

Qlop also misses about 3% of merges compared to {gen,em}lop, for some other reason.

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
be prepended with `(?-i)` should case-sensitivity ever be needed. qlop only supports plaintext
whole-word matching.

Genlop and qlop use a single flag for min/max date, so it isn't possible to specify only a max date.

|                                                        | genlop | qlop  | emlop  |
| :----------------------------------------------------- | :----: | :---: | :----: |
| Limit log parsing by date                              | yes    | yes   | yes    |
| Plaintext exact package search                         | yes    | yes   | yes    |
| Regexp package search                                  | yes    | no    | yes    |
| Regexp case-sensitivity switch                         | flag   | n/a   | syntax |
| Default search mode                                    | plain  | plain | regexp |

## Merge time prediction

Emlop uses only the last 10 merges (configurable) for predictions, which makes a big difference if
you have a long emerge history and a package progressivley takes longer to compile (for example
chromium) or if you got a hardware upgrade.

Emlop takes elapsed time into account for `emerge -p` predictions, so the ETA stays accurate
throughout a long merge.

Qlop only predicts the current merge. When run as a normal user, it warns about missing /proc
permissions, finds bogus current merges, and doesn't give the same ETA for the ones it finds.

All tools give pessimistic prediction when packages are merged in parallel, because they assume
sequential merging. Even if they detected an ongoing parallel merge, it's not clear how they would
estimate the resulting speedup factor.

|                                                          | genlop   | qlop     | emlop         |
| :------------------------------------------------------- | :------: | :------: | :-----------: |
| Show current merge                                       | yes      | yes      | yes           |
| Show current merge ETA                                   | yes      | yes      | yes           |
| Show current merge stage                                 | no       | no       | no            |
| Show `emerge -p` merges global ETA                       | yes      | no       | yes           |
| Show `emerge -p` merges individual ETAs                  | no       | no       | yes           |
| Global/current ETA format                                | duration | duration | duration+date |
| Accuracy of time estimation                              | ok       | ok       | good          |
| Query gentoo.linuxhowtos.org for unknown packages        | yes      | no       | no            |

## Speed

Here are timings for some common commands (in milliseconds, 95th centile of 50 runs, output to
Alacritty terminal) measured using `benches/exec_compare.rs`, on an i7-9750H with an SSD and an
emerge.log with ~11K merges.

The commands were selected to be comparable, but Some differences do influence timings: emlop always
show merge time and package version in "log" mode, which takes some more work. Filtering by
plaintext isn't noticeably faster than by case-(in)sensitive regexp ({gen,em}lop only).

|                                                               | genlop | qlop | emlop |
| :-------------------------------------------------------------| -----: | ---: | ----: |
| `genlop -l; qlop -m; emlop l`                                 |    385 |   52 |    24 |
| `genlop -lut; qlop -muUvt; emlop l -smu`                      |    600 |   72 |    42 |
| `genlop -e gcc; qlop gcc; emlop l -e gcc`                     |    322 |   20 |    14 |
| `genlop -tlu gcc; qlop -tvmuU gcc; emlop l -smu -e gcc`       |    606 |   24 |    17 |
| `emerge dummybuild&;genlop -c;qlop -r;emlop p`                |    355 |   34 |    24 |
| `genlop -p < emerge-p.gcc.out; emlop p < emerge-p.gcc.out`    |    353 |  n/a |    23 |
| `genlop -p < emerge-p.qt.out;  emlop p < emerge-p.qt.out`     |   3380 |  n/a |    25 |
| `genlop -p < emerge-p.kde.out; emlop p < emerge-p.kde.out`    |  21047 |  n/a |    25 |

Emlop is faster than qlop, which is much faster than genlop. None of the timings are showstoppers on
this fast machine, except for `emerge -p` ETA (not implemented by qlop) which is prohibitively slow
in genlop.

## misc

|                                                       | genlop | qlop   | emlop         |
| :---------------------------------------------------- | :----: | :----: | :-----------: |
| Shell completion                                      | bash   | none   | bash/zsh/fish |
| An ebuild in the gentoo portage tree                  | yes    | yes    | no            |
| Unittests                                             | no     | yes    | yes           |
| Documentation and help                                | ok     | good   | good          |
| Development activity                                  | mature | active | active        |
