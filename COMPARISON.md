# Comparison with genlop, qlop, pqlop, golop

[Genlop](https://github.com/gentoo-perl/genlop) is great and was the inspiration for emlop. Original
motivation for a rewrite was improving the speed and accuracy of `genlop -p`, and learning
Rust. Other rewrites exists: [qlop](https://github.com/gentoo/portage-utils) (part of the
[q-applets](https://wiki.gentoo.org/wiki/Q_applets) toolkit),
[Pqlop](https://bitbucket.org/LK4D4/pqlop), and [golop](https://github.com/klausman/golop). Perl,
Python, C, Go, Rust... at least Gentoo doesn't suffer from a language monoculture ;)

This file compares `genlop-0.30.10`, `qlop-0.89`, and `emlop-latest` (2020-09-13). `pqlop` and
`golop` are no longer included in this comparison due to being abandonned. Please report any
outdated/incorrect info using [the issue tracker](https://github.com/vincentdephily/emlop/issues).


## Interface

Emlop is organised into subcommands, whereas {gen,q}lop only use (possibly conflicting) flags. It
tries to merge functions where that makes sense, for example `emlop l` combines `genlop -l`, `genlop
-e`, and `genlop -t`, because there didn't seem to be a point to separate them. Same thing with
`genlop -c` and `genlop -p` which are combined into `emlop p`.

## Output

Emlop output aims to be compact, beautiful, and easy to read/parse. Qlop is very close to genlop,
but did make some outputs more compact.

Default qlop duration output depends on length: `45s` -> `3′45″` -> `1:23:45`. Machine output
applies to dates and durations at the same time.

|                                                   | genlop | qlop       | emlop   |
| :------------------------------------------------ | :----: | :--------: | :-----: |
| Output density                                    | sparse | medium     | compact |
| Colorized output                                  | yes    | yes        | yes     |
| Date output options                               | utc    | iso,unix   | -       |
| Duration output style (seconds,hh:mm:ss,text)     | text   | hms,s,text | hms,s   |
| Aligned output                                    | some   | some       | all     |
| Headers                                           | no     | no         | no      |

## Merge log

|                                                       | genlop | qlop  | emlop |
| :---------------------------------------------------- | :----: | :---: | :---: |
| Display merges                                        | yes    | yes   | yes   |
| Display syncs                                         | yes    | yes   | yes   |
| Display unmerges                                      | yes    | yes   | yes   |
| Distinguish autoclean/manual unmerges                 | no     | yes   | no    |
| Display interrupted merges                            | no     | no    | no    |
| Display currently installed package's USE/CFLAGS/date | yes    | no    | no    |
| Display merge begin time or end time                  | end    | begin | end   |

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

|                                                        | genlop | qlop  | emlop  |
| :----------------------------------------------------- | :----: | :--:  | :----: |
| Limit log parsing by date                              | yes    | yes   | yes    |
| Plaintext exact package search                         | yes    | yes   | yes    |
| Regexp package search                                  | yes    | no    | yes    |
| Regexp case-sensitivity switch                         | flag   | n/a   | syntax |
| Default search mode                                    | plain  | plain | regexp |
| Unfiltered package listing                             | yes    | yes   | yes    |

## Merge time prediction

Qlop only predicts the current merge. When run as a normal user, it warns about missing /proc
permissions, doesn't give the same ETA, and finds bogus current merges.

`emlop p` uses only the last 10 merges (configurable) for predictions, which makes a big difference
if you have a long emerge history and a package progressivley takes longer to compile (for example
chromium) or if you got a hardware upgrade.

`emlop p` takes elapsed time into account for `emerge -p` predictions, so the ETA stays accurate
throughout a long merge.

All tools give pessimistic prediction (if any) when packages are merged in parallel, because they
assume sequential merging. Even if they detected an ongoing parallel merge, it's not clear how they
would estimate the resulting speedup factor.

|                                                          | genlop   | qlop     | emlop         |
| :------------------------------------------------------- | :------: | :------: | :-----------: |
| Show current merge                                       | yes      | yes      | yes           |
| Show current merge ETA                                   | yes      | yes      | yes           |
| Show current merge stage                                 | no       | no       | no            |
| Show `emerge -p` merges global ETA                       | yes      | no       | yes           |
| Show `emerge -p` merges individual ETAs                  | no       | no       | yes           |
| Global/current ETA format                                | duration | duration | duration+date |
| Accuracy of time estimation                              | ok       | ?        | good          |
| Query gentoo.linuxhowtos.org for unknown packages        | yes      | no       | no            |

## Speed

Here are timings for some common commands (in seconds, 95th centile of 25 runs, output to Alacritty
terminal, ~40K emerges in emerge.log, SSD, Intel i7-4800MQ) measured using
`benches/exec_compare.crs`.

The commands were selected to be comparable, but Some differences do influence timings: {em,go}lop
always calculate the merge time in "log" mode, which takes some more work. {q,pq}lop don't calculate
the ETA in "current merge" mode, which takes much less work. Filtering by plaintext isn't noticeably
faster than by case-(in)sensitive regexp ({gen,em}lop only).

|                                                               | genlop | qlop | emlop |
| :-------------------------------------------------------------| -----: | ---: | ----: |
| `genlop -l; qlop -l; emlop l`                                 |   2.21 | 0.42 |  0.35 |
| `genlop -t gcc; qlop -g gcc; emlop l -e gcc`                  |   1.55 | 0.11 |  0.14 |
| `genlop -e gcc; qlop -l gcc; emlop l -e gcc`                  |   1.26 | 0.11 |  0.14 |
| `MAKEOPTS=-j1 emerge -O1 firefox &;genlop -c;qlop -c;emlop p` |   1.57 | 0.00 |  0.20 |
| `genlop -c;qlop -c;emlop p`                                   |   0.70 | 0.00 |  0.01 |
| `genlop -p < emerge-p.gcc.out; emlop p < emerge-p.gcc.out`    |   1.48 | n/a  |  0.18 |
| `genlop -p < emerge-p.qt.out;  emlop p < emerge-p.qt.out`     |  28.75 | n/a  |  0.18 |
| `genlop -p < emerge-p.kde.out; emlop p < emerge-p.kde.out`    | 196.37 | n/a  |  0.18 |

Emlop and Qlop are similarly fast. The others are slower but not showstoppers, except for `genlop
-p` which is muuuch slower than `emlop p` (while qlop doesn't implement the feature).

Some bugs found while benching on my system: `qlop -g gcc` misses 2 merges. The emerge logs look
fine and {gen,em}lop agree with each other.

## misc

Genlop is the original from 2007; mature but development has stoped in 2015. Qlop started in 2011 as
part of the broader portage-utils; is is mature and maintained. Emlop started in december 2017, it
is mature and still adding features. Pqlop and Golop started in 2011 and 2017 respectively, but seem
to be abandonned experiments.

|                                                       | genlop | qlop   | emlop  |
| :---------------------------------------------------- | :----: | :----: | :----: |
| Bash completion                                       | yes    | no     | no     |
| An ebuild in the gentoo portage tree                  | yes    | yes    | no     |
| Support for non-Linux platforms                       | yes    | yes    | no     |
| Unittests                                             | no     | yes    | yes    |
| Documentation and help                                | ok     | good   | good   |
| Development activity                                  | mature | active | active |

Emlop cannot yet detect current emerge processes on non-Linux; I'm taking an educated guess for the
other tools.
