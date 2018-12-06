# Comparison with genlop, qlop, pqlop, golop

[Genlop](https://github.com/gentoo-perl/genlop) is great and was the inspiration for emlop. Original
motivation for a rewrite was improving the speed and accuracy of `genlop -p`, and learning
Rust. Other rewrites exists: [qlop](https://github.com/gentoo/portage-utils) (part of the
[q-applets](https://wiki.gentoo.org/wiki/Q_applets) toolkit),
[Pqlop](https://bitbucket.org/LK4D4/pqlop), and [golop](https://github.com/klausman/golop). Perl,
Python, C, Go, Rust... at least Gentoo doesn't suffer from a language monoculture ;)

This file lists some differences between the various tools. The goal is to help users pick the right
tool today, and help emlop become the best tool tomorrow.

Be sure to check the other projects for updates, and notify me if any info here is wrong, missing,
or outdated.


## Interface

Emlop is organised into subcommands, whereas {gen,q,go,pq}lop only use (possibly conflicting)
flags. It tries to merge functions where that makes sense, for example `emlop l` combines `genlop
-l`, `genlop -e`, and `genlop -t`, because there didn't seem to be a point to separate them. Same
thing with `genlop -c` and `genlop -p` which are combined into `emlop p`.

## Output

Emlop output aims to be compact, beautiful, and easy to read/parse. Qlop is very close to genlop,
but did make some outputs more compact. Pqlop has a sparse, colored output very close (often
identical) to genlop. Golop has a fairly spartan look: compact, machine-like, no color.

|                                                   | genlop | qlop   | emlop   | pqlop  | golop   |
| :------------------------------------------------ | :----: | :----: | :-----: | :----: | :-----: |
| Output density                                    | sparse | medium | compact | sparse | compact |
| Colorized output                                  | yes    | yes    | yes     | yes    | no      |
| Date output options                               | utc    | -      | -       | -      | -       |
| Duration output style (seconds,hh:mm:ss,text)     | text   | s,text | hms,s   | text   | hms     |
| Aligned output                                    | some   | all    | all     | some   | some    |
| Headers                                           | no     | no     | no      | no     | some    |

## Merge log

|                                                       | genlop | qlop    | emlop | pqlop | golop |
| :---------------------------------------------------- | :----: | :-----: | :---: | :---: | :---: |
| Display merges                                        | yes    | yes     | yes   | yes   | yes   |
| Display syncs                                         | yes    | yes     | yes   | buggy | no    |
| Display unmerges                                      | yes    | yes     | no    | yes   | no    |
| Display interrupted merges                            | no     | no      | no    | yes   | no    |
| Display currently installed package's USE/CFLAGS/date | yes    | no      | no    | no    | no    |
| Display merge begin time or end time                  | end    | depends | end   | begin | begin |

Pqlop doesn't handle newer log format for sync events. Qlop displays begin time when it displays
duration, end time otherwise.

## Merge stats

Emlop has a dedicated stats mode, whereas {q,pq,go}lop show aggregate data in various places, often
after sequential data.

|                                                          | genlop | qlop | emlop | pqlop | golop |
| :------------------------------------------------------- | :----: | :--: | :---: | :---: | :---: |
| Individual merge count/total/average/prediction          | c,t,a  | c,a  | c,t,p | c     | c,a   |
| Total merge count/total/average                          | -      | -    | c,t,a | -     | c     |
| Total sync count/total/average                           | -      | -    | c,t,a | c,a   | -     |
| Group stats by year/month/week/day                       | no     | no   | any   | one   | no    |

Pqlop gives the overall number of syncs per week. It tries to give the total sync time, but that's
buggy.

## Filtering

Genlop switches case-sensitivity using `-s` vs `-S` flag. Emlop doesn't have a flag, but regexp can
be prepended with `(?-i)` should case-sensitivity ever be needed. {q,pq,go}lop only support
plaintext whole-word matching.

Pqlop requires a search term, it can only display info about a particular package. Golop only
displays one of the possible matches when an ambiguous name is given (like `pkgconfig`).

|                                                        | genlop | qlop  | emlop  | pqlop | golop |
| :----------------------------------------------------- | :----: | :--:  | :----: | :---: | :---: |
| Limit log parsing by date                              | yes    | yes   | yes    | no    | no    |
| Plaintext exact package search                         | yes    | yes   | yes    | yes   | yes   |
| Regexp package search                                  | yes    | no    | yes    | no    | no    |
| Regexp case-sensitivity switch                         | flag   | n/a   | syntax | n/a   | n/a   |
| Default search mode                                    | plain  | plain | regexp | plain | plain |
| Unfiltered package listing                             | yes    | yes   | yes    | no    | yes   |

## Merge time prediction

Qlop and pqlop don't do any merge time prediction, and golop only predicts the current merge.

`emlop p` uses only the last 10 merges (configurable) for predictions, which makes a big difference
if you have a long emerge history and a package progressivley takes longer to compile (for example
chromium) or if you got a hardware upgrade.

`emlop p` takes elapsed time into account for `emerge -p` predictions, so the ETA stays accurate
throughout a long merge.

All tools give pessimistic prediction (if any) when packages are merged in parallel, because they
assume sequential merging. Even if they detected an ongoing parallel merge, it's not clear how they
would estimate the resulting speedup factor.

|                                                          | genlop | qlop | emlop | pqlop | golop |
| :------------------------------------------------------- | :----: | :--: | :---: | :---: | :---: |
| Show current merge                                       | yes    | yes  | yes   | yes   | yes   |
| Show current merge ETA                                   | yes    | no   | yes   | no    | yes   |
| Show current merge stage                                 | no     | no   | no    | yes   | no    |
| Show `emerge -p` merges global ETA                       | yes    | no   | yes   | no    | no    |
| Show `emerge -p` merges individual ETAs                  | no     | no   | yes   | no    | no    |
| Accuracy of time estimation                              | ok     | n/a  | good  | n/a   | ok    |
| Query gentoo.linuxhowtos.org for unknown packages        | yes    | n/a  | no    | n/a   | no    |

## Speed

Here are timings for some common commands (in seconds, 95th centile of 25 runs, output to Alacritty
terminal, ~40K emerges in emerge.log, SSD, Intel i7-4800MQ) measured using
`benches/exec_compare.crs`.

The commands were selected to be comparable, but Some differences do influence timings: {em,go}lop
always calculate the merge time in "log" mode, which takes some more work. {q,pq}lop don't calculate
the ETA in "current merge" mode, which takes much less work. Filtering by plaintext isn't noticeably
faster than by case-(in)sensitive regexp ({gen,em}lop only).

|                                                                                 | genlop | qlop | emlop | pqlop | golop |
| :------------------------------------------------------------------------------ | -----: | ---: | ----: | ----: | ----: |
| `genlop -l; qlop -l; emlop l; golop`                                            |   2.21 | 0.42 |  0.35 |   n/a |  1.79 |
| `genlop -t gcc; qlop -g gcc; emlop l -e gcc; pqlop -g gcc; golop -t gcc`        |   1.55 | 0.11 |  0.14 |  0.57 |  1.60 |
| `genlop -e gcc; qlop -l gcc; emlop l -e gcc; pqlop -l gcc; golop -t gcc`        |   1.26 | 0.11 |  0.14 |  0.58 |  1.61 |
| `MAKEOPTS=-j1 emerge -O1 firefox &;genlop -c;qlop -c;emlop p;pqlop -c;golop -c` |   1.57 | 0.00 |  0.20 |  0.59 |  1.95 |
| `genlop -c;qlop -c;emlop p;pqlop -c;golop -c`                                   |   0.70 | 0.00 |  0.01 |  0.57 |  1.64 |
| `genlop -p < emerge-p.gcc.out; emlop p < emerge-p.gcc.out`                      |   1.48 | n/a  |  0.18 |   n/a |   n/a |
| `genlop -p < emerge-p.qt.out;  emlop p < emerge-p.qt.out`                       |  28.75 | n/a  |  0.18 |   n/a |   n/a |
| `genlop -p < emerge-p.kde.out; emlop p < emerge-p.kde.out`                      | 196.37 | n/a  |  0.18 |   n/a |   n/a |

Emlop and Qlop are similarly fast. The others are slower but not showstoppers, except for `genlop
-p` which is muuuch slower than `emlop p` (while {q,pq,go}lop don't implement the feature).

Some bugs found while benching on my system: `qlop -g gcc` misses 2 merges, `golop -t gcc` misses 5,
and `golop` misses 2.5% of merges. The emerge logs look fine and {gen,em,pq}lop agree with each
other. `golop -c` often [doesn't detect running emerge](https://github.com/klausman/golop/issues/1).

## misc

Genlop is the original from 2007; mature but development has stoped in 2015. Qlop started in 2011 as
part of the broader portage-utils; is is less featureful than genlop/emlop, but mature and
maintained. Emlop started in december 2017, it is now mature but still adding features. Pqlop and
Golop started in 2011 and 2017 respectively, but seem to be abandonned experiments.

|                                                       | genlop | qlop   | emlop  | pqlop | golop |
| :---------------------------------------------------- | :----: | :----: | :----: | :---: | :---: |
| Bash completion                                       | yes    | no     | no     | no    | no    |
| An ebuild in the gentoo portage tree                  | yes    | yes    | no     | yes   | yes   |
| Support for non-Linux platforms                       | yes    | yes    | no     | yes   | ?     |
| Unittests                                             | no     | yes    | yes    | no    | no    |
| Documentation and help                                | ok     | good   | good   | poor  | ok    |
| Development activity                                  | dead   | mature | active | dead  | dead  |

Emlop cannot yet detect current emerge processes on non-Linux; I'm taking an educated guess for the
other tools.
