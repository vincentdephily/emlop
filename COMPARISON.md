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

Pqlop has a colorful output that is very close (often identical) to genlop. Qlop is also very close
to genlop, but did make some outputs more compact. Emlop and golop have a more spartan look: more
compact, more machine-like, less colorful.

|                                                   | genlop | qlop   | emlop   | pqlop  | golop   |
| :------------------------------------------------ | :----: | :----: | :-----: | :----: | :-----: |
| Output density                                    | sparse | medium | compact | sparse | compact |
| Colorized output                                  | yes    | yes    | no      | yes    | no      |
| Date output options (Utc)                         | utc    | -      | -       | -      | -       |
| Duration output style (seconds,hh:mm:ss,text)     | text   | s,text | hms     | text   | hms     |
| Headers                                           | no     | no     | no      | no     | some    |

## Merge history

Emlop has a specific mode deticated to stats whereas {q,pq,go}lop include that at the end of other
outputs.

|                                                                    | genlop | qlop | emlop | pqlop | golop |
| :----------------------------------------------------------------- | :----: | :--: | :---: | :---: | :---: |
| Display sync and unmerges                                          | yes    | yes  | no    | yes   | no    |
| Display interrupted merges                                         | no     | no   | no    | yes   | no    |
| Display info about currently installed package like USE, CFLAGS... | yes    | no   | no    | no    | no    |
| Display extra merge stats like total time/count, average...        | no     | yes  | yes   | yes   | yes   |

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

## Speed

Here are timings for some common commands (in seconds, best time of many runs, ~35K emerges in
emerge.log).

Some timing-related feature differences: {em,go}lop always calculate the merge time in "list"
mode. {q,pq}lop don't calculate the ETA in "current merge" mode. Filtering by plaintext isn't
noticeably faster than by case-(in)sensitive regexp ({gen,em}lop only).

|                                                                          | genlop | qlop | emlop | pqlop | golop |
| :----------------------------------------------------------------------- | -----: | ---: | ----: | ----: | ----: |
| `genlop -l; qlop -l; emlop l; golop`                                     | 0.9    | 0.28 | 0.30  | n/a   | 1.0   |
| `genlop -t gcc; qlop -g gcc; emlop l -e gcc; golop -t gcc; pqlop -g gcc` | 0.5    | 0.05 | 0.09  | 0.8   | 0.3   |
| `genlop -e gcc; qlop -l gcc; emlop l -e gcc; golop -t gcc; pqlop -l gcc` | 0.3    | 0.06 | 0.09  | 0.8   | 0.3   |
| `emerge -O1 firefox &;genlop -c;qlop -c;emlop p;pqlop -c;golop -c`       | 0.6    | 0.00 | 0.12  | 0.3   | 0.9   |
| `genlop -p < emerge-p.gcc.out; emlop p < emerge-p.gcc.out`               | 0.5    | n/a  | 0.12  | n/a   | n/a   |
| `genlop -p < emerge-p.qt.out;  emlop p < emerge-p.qt.out`                | 14.3   | n/a  | 0.12  | n/a   | n/a   |
| `genlop -p < emerge-p.kde.out; emlop p < emerge-p.kde.out`               | 99.3   | n/a  | 0.12  | n/a   | n/a   |

Qlop is fastest, followed closely by emlop. The others are slower but not showstoppers, except for
`genlop -p` which is muuuch slower than `emlop p` (while {q,pq,go}lop don't implement the feature).

Some bugs found while benching on my system: `qlop -g gcc` misses 2 merges, `golop -t gcc` misses 5,
and `golop` misses 2.5% of merges. The emerge logs look fine and {gen,em,pq}lop agree with each
other. `golop -c` often [doesn't detect running emerge](https://github.com/klausman/golop/issues/1).

## Merge time prediction

Qlop and pqlop don't do any merge time prediction, and golop only predicts the current ebuild.

`emlop p` uses only the last 10 merges (configurable) for predictions, which makes a big difference
if you have a long emerge history and a package progressivley takes longer to compile (for example
chromium) or if you got a hardware upgrade.

`emlop p` checks currently runing emerges even in `emerge -p` mode, to deducts the elapsed time from
its estimate.

All tools give pessimistic prediction (if any) when merging multiple packages in parallel, because
they assume sequential merging. Even if they detected an ongoing parallel merge, it's not clear how
they would estimate the resulting speedup factor.

|                                                          | genlop | qlop | emlop | pqlop | golop |
| :------------------------------------------------------- | :----: | :--: | :---: | :---: | :---: |
| Show current merges                                      | yes    | yes  | yes   | yes   | yes   |
| Predict from current merges                              | yes    | no   | yes   | no    | yes   |
| Predict from `emerge -p`                                 | yes    | no   | yes   | no    | no    |
| Display individial package estimates for `emerge -p`     | no     | n/a  | yes   | n/a   | n/a   |
| Take current merges into account for `emerge -p`         | no     | n/a  | yes   | n/a   | n/a   |
| Accuracy of time estimation                              | ok     | n/a  | good  | n/a   | ?     |
| Query gentoo.linuxhowtos.org for unknown packages        | yes    | n/a  | no    | n/a   | no    |

## misc

Genlop started in 2007 but development seem to have stoped in 2015. Pqlop saw development between
2011 and 2012, and a lone bugfix in 2016. Portage-utils (qlop) development has slowed down but this
is probably more a sign of maturity than abandonment. Golop development started and seemingly ended
in december 2017. Emlop started around the same time as golop, and has seen regular progress so far
(of course if emlop development stops, this comparison will become stale).

|                                                          | genlop | qlop | emlop | pqlop | golop |
| :------------------------------------------------------- | :----: | :--: | :---: | :---: | :---: |
| Bash completion                                          | yes    | no   | no    | no    | no    |
| An ebuild in the gentoo portage tree                     | yes    | yes  | no    | yes   | yes   |
| Support for non-Linux platforms                          | yes    | yes  | no    | yes   | ?     |
| Unittests                                                | no     | yes  | yes   | no    | no    |
| Documentation and help                                   | ok     | good | good  | poor  | ok    |
| Active development                                       | no     | yes  | yes   | no    | no ?  |

Emlop cannot yet detect current emerge processes on non-Linux; I'm taking an educated guess for the
other tools.
