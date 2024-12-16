# EMerge LOg Parser

Emlop parses emerge logs (as generated by [portage](https://wiki.gentoo.org/wiki/Project:Portage),
the [Gentoo](https://www.gentoo.org/) package manager) to yield useful info like merge history and
merge time prediction.

It draws inspiration from [genlop](https://github.com/gentoo-perl/genlop) and
[qlop](https://github.com/gentoo/portage-utils) but aims to be faster, more accurate, and more
ergonomic, see [comparison](docs/COMPARISON.md).

## Usage

Emlop is split into commands. Command names and arguments can be abbreviated (so `emlop log --from
'1 day' --duration human` is the same as `emlop l -f1d --dur h`), and shell completion is
available. See `emlop --help` and `emlop <command> --help` for complete and up to date usage info.

### Common options

All commands share these arguments, affecting parsing and output:

    Options:
      -F, --logfile <file>  Location of emerge log file
      -v...                 Increase verbosity (can be given multiple times)
      -h, --help            Print help (see more with '--help')
      -V, --version         Print version
    Filter:
      -f, --from <date>  Only parse log entries after <date>
      -t, --to <date>    Only parse log entries before <date>
    Format:
      -H, --header [<bool>]    Show table header
          --duration <format>  Output durations in different formats
          --date <format>      Output dates in different formats
          --utc [<bool>]       Parse/display dates in UTC instead of local time
          --color [<bool>]     Enable color (yes/no/auto)
      -o, --output <format>    Ouput format (columns/tab/auto)

### List merges, unmerges, and syncs  with `log`

![Log demo](log.webp)

Log-specific options:

    Format:
          --starttime [<bool>]  Display start time instead of end time
    Filter:
      [search]...           Show only packages/repos matching <search>
      -e, --exact           Match <search> using plain string
      -s, --show <m,u,s,a>  Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll
      -N, --first [<num>]   Show only the first <num> entries
      -n, --last [<num>]    Show only the last <num> entries

Note that `emaint sync` currently [doesn't write to emerge.log](https://bugs.gentoo.org/553788), so
`emlop l --show s` will appear empty if you use `emaint`. Use `emerge --sync` or `eix-sync` instead.

### Estimate how long a merge with take with `predict`

![Predict demo](predict.webp)

Predict-specific arguments:

    Options:
          --tmpdir <dir>    Location of portage tmpdir
    Filter:
      -s, --show <e,m,t,a>     Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll
      -N, --first [<num>]      Show only the first <num> entries
      -n, --last [<num>]       Show only the last <num> entries
          --resume [<source>]  Use main, backup, either, or no portage resume list
    Stats:
          --limit <num>     Use the last <num> merge times to predict durations
          --avg <fn>        Select function used to predict durations
          --unknown <secs>  Assume unkown packages take <secs> seconds to merge

### Show aggregated statistics with `stats`

![Stats demo](stats.webp)

Stats-specific arguments:

    Filter:
      [search]...           Show only packages/repos matching <search>
      -e, --exact           Match <search> using plain string
      -s, --show <p,t,s,a>  Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll
    Stats:
      -g, --groupby <y,m,w,d,n>  Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one
          --limit <num>          Use the last <num> merge times to predict durations
          --avg <fn>             Select function used to predict durations

### Other commands

* `complete`: shell completion helper
* `accuracy`: analize predictions accuracy

### Configuration file

![Config demo](config.webp)

Emlop reads default settings from `$HOME/.config/emlop.toml`. Set `$EMLOP_CONFIG` env var to change
the file location, or set it to  `""` to disable.

This [example file](emlop.toml) documents the format, and lists supported options. Command-line
arguments take precedence over the config file.

## Installation

### Using portage

    emerge emlop

The ebuild is also maintained in the [moltonel](https://github.com/vincentdephily/moltonel-ebuilds)
overlay, which you can enable using
[eselect-repository](https://wiki.gentoo.org/wiki/Eselect/Repository).

### Using cargo

Install Rust and using [portage](https://wiki.gentoo.org/wiki/Rust) or
[rustup](https://www.rust-lang.org/en-US/install.html). Make sure `~/.cargo/bin/`, is in your
`$PATH`.

The current Minimum Supported Rust Version is 1.74. When building `emlop` with an old rustc version,
you might need to pass `--locked` to `cargo install`, to use explicitly tested dependency versions.

#### From crates.io

    cargo install -f emlop

#### From git

    git clone https://github.com/vincentdephily/emlop
    cd emlop
    cargo test
    cargo install -f --path .

#### Misc files

Cargo only installs the binary, which is all you really need, but you may want to manualy install
some files fetched from [github](https://github.com/vincentdephily/emlop) or the [crates.io
page](https://crates.io/crates/emlop): [bash completion](completion.bash), [zsh
completion](completion.zsh), [fish completion](completion.fish), and [example config
file](emlop.toml).

## Contributing

Thanks in advance. See [contributing](docs/CONTRIBUTING.md) for pointers. Emlop is licensed as GPLv3.
