# master

* Make regexp search case-insensitive: no need to remeber that 'PyQt' is camelcase anymore.
* Add `--exact`/`-e` (non-regexp) search mode (like the default/only mode of `{gen,q,pq,go}lop`).

# 0.1 2018-03-07

First release, huzza !

The core commands `list`, `predict`, ans `stats` are here and emlop already feels like a worthy
replacement for genlop (by being faster) or qlop (by having a predict mode and regexp search).

Thanks to singul0 for fixing a panic when outputing to a closed pipe.
