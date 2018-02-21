# Comparison with genlop, pqlop, golop

[Genlop](https://github.com/gentoo-perl/genlop) is great and was the inspiration for emlop. Original
motivation for a rewrite was improving the speed and accuracy or `genlop -p`, and learning
Rust. [Pqlop](https://bitbucket.org/LK4D4/pqlop) and [golop](https://github.com/klausman/golop) are
two other genlop rewrites, in python and go respectively.

This file lists some differences between the various tools. The goal is to help users pick the right
tool today, and help emlop become the best tool tomorrow.

Be sure to check the other projects for updates, and notify me if any info here is wrong, missing,
or outdated.


## Interface

Emlop is organised into subcommands, whereas {gen,go,pq}lop only use (possibly conflicting)
flags. It tries to merge functions where that makes sense, for example `emlop l` combines `genlop
-l`, `genlop -e`, and `genlop -t`, because there didn't seem to be a point to separate them. Same
thing with `genlop -c` and `genlop -p` which are combined into `emlop p`.

## Output

Pqlop has a colorful output that is very close (often identical) to genlop. Emlop and golop generaly
aim for a more spartan look: more compact, more machine-like, less colorful.

|                                                                    | genlop | emlop   | pqlop | golop   |
| :----------------------------------------------------------------- | :----: | :-----: | :---: | :-----: |
| Output style                                                       | fancy  | compact | fancy | compact |
| Colorized output                                                   | yes    | no      | yes   | no      |
| Select output timezone                                             | yes    | no      | no    | no      |
| Headers                                                            | no     | no      | no    | some    |

## Merge history

Emlop has a specific mode deticated to stats whereas pqlop and golop include that at the end of
other outputs.

|                                                                    | genlop | emlop   | pqlop | golop   |
| :----------------------------------------------------------------- | :----: | :-----: | :---: | :-----: |
| Display sync and unmerges                                          | yes    | no      | yes   | no      |
| Display interrupted merges                                         | no     | no      | yes   | no      |
| Display info about currently installed package like USE, CFLAGS... | yes    | no      | no    | no      |
| Display extra merge stats like total time/count, average...        | no     | yes     | yes   | yes     |

## Filtering

Pqlop requires a search term, it can only display info about a particular package.

|                                                                    | genlop | emlop   | pqlop | golop   |
| :----------------------------------------------------------------- | :----: | :-----: | :---: | :-----: |
| Limit log parsing by date                                          | yes    | no      | no    | no      |
| Case-sensitive regexp package search                               | yes    | yes     | no    | no      |
| Case-insensitive regexp package search                             | yes    | no      | no    | no      |
| Plaintext exact package search                                     | yes    | no      | yes   | yes     |
| Unfiltered package listing                                         | yes    | yes     | no    | yes     |

## Speed

Emlop is faster (and uses less memory, as a bonus). Here are timings for some common commands (in
seconds, best time of many runs, ~35K emerges in emerge.log):

|                                                                    | genlop | emlop   | pqlop | golop   |
| :----------------------------------------------------------------- | :----: | :-----: | :---: | :-----: |
| `genlop -l; emlop l; golop`                                        | 0.9    | 0.4     | n/a   | 1.0     |
| `genlop -t gcc; emlop l gcc$; golop -t gcc; pqlop -g gcc`          | 0.5    | 0.2     | 0.8   | 0.3     |
| `genlop -e gcc; emlop l gcc$; golop -t gcc; pqlop -l gcc`          | 0.3    | 0.2     | 0.8   | 0.3     |
| `emerge -Op gcc > p;              genlop -p < p; emlop p < p`      | 0.5    | 0.2     | n/a   | n/a     |
| `emerge -Op $(eix -ICc# qt) > p;  genlop -p < p; emlop p < p`      | 13.3   | 0.2     | n/a   | n/a     |
| `emerge -Op $(eix -ICc# kde) > p; genlop -p < p; emlop p < p`      | 92.6   | 0.2     | n/a   | n/a     |

Emlop and golop always calculate the merge time. Pqlop and golop do not have an `emerge -p` mode.


## Merge time prediction

`emlop p` uses only the last 10 merges (configurable) for predictions, which makes a big difference
if you have a long emerge history and a package progressivley takes longer to compile (for example
chromium) or if you got a hardware upgrade.

`emlop p` checks currently runing emerges even in `emerge -p` mode, to deducts the elapsed time from
its estimate.

All four tools will give pessimistic prediction when merging multiple packages in parallel, because
they assume sequential merging. Even if they detected an ongoing parallel merge, it's not clear how
they would estimate the resulting speedup factor.

|                                                                    | genlop | emlop   | pqlop | golop   |
| :----------------------------------------------------------------- | :----: | :-----: | :---: | :-----: |
| Show current merges                                                | yes    | yes     | yes   | yes     |
| Predict from current merges                                        | yes    | yes     | no    | yes     |
| Predict from `emerge -p`                                           | yes    | yes     | no    | no      |
| Display individial package estimates for `emerge -p`               | no     | yes     | n/a   | n/a     |
| Take current merges into account for `emerge -p`                   | no     | yes     | n/a   | n/a     |
| Accuracy of time estimation                                        | ok     | good    | n/a   | ?       |
| Query gentoo.linuxhowtos.org for unknown packages                  | yes    | no      | no    | no      |

## misc

Genlop started in 2007 but development seem to have stoped in 2015. Pqlop saw development between
2011 and 2012, and a lone bugfix in 2016. Golop started in december 2017, but doesn't seem to have
evolved since. Emlop started around the same time as golop, and has seen regular progress so far.

Support for non-Linux is (probably only) tied to detecting currently running merges.

|                                                                    | genlop | emlop   | pqlop | golop   |
| :----------------------------------------------------------------- | :----: | :-----: | :---: | :-----: |
| Bash completion                                                    | yes    | no      | no    | no      |
| An ebuild in the gentoo portage tree                               | yes    | no      | yes   | yes     |
| Support for non-Linux platforms                                    | yes    | no      | ?     | ?       |
| A growing set of unittests                                         | no     | yes     | no    | no      |
| Documentation and help                                             | ok     | good    | poor  | ok      |
| Active development                                                 | no     | yes     | no    | yes ?   |
