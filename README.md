# EMerge LOg Parser

Emlop parses emerge logs (as generated by [portage](https://wiki.gentoo.org/wiki/Project:Portage),
the [Gentoo](https://www.gentoo.org/) package manager) to yield useful info like merge history and
merge time prediction.

It draws inspiration from [genlop](https://github.com/gentoo-perl/genlop) and
[qlop](https://github.com/gentoo/portage-utils) but aims to be faster, more accurate, and more
ergonomic, see [comparison](docs/COMPARISON.md).

## Usage

Emlop is split into commands. Command names and arguments can be abbreviated, and shell completion
is available. See `emlop --help` and `emlop <sucommand> --help` for complete and up to date usage
info.

### Common options

All commands share these arguments, affecting parsing and output:

    Options:
      -F, --logfile <file>  Location of emerge log file [default: /var/log/emerge.log]
      -v...                 Increase verbosity (can be given multiple times)
      -h, --help            Print help (see more with '--help')
      -V, --version         Print version
    Filter:
      -f, --from <date>  Only parse log entries after <date>
      -t, --to <date>    Only parse log entries before <date>
    Format:
      -H, --header             Show table header
          --utc                Parse/display dates in UTC instead of local time
      -o, --output <format>    Ouput format (cols/c/tab/t)
          --duration <format>  Output durations in different formats [default: hms]
          --date <format>      Output dates in different formats [default: ymdhms]
          --color [<when>]     Enable color (always/never/y/n)


### List merges, unmerges, and syncs  with `log`

![Log demo](log.webp)

Log-specific options:

    Filter:
      [search]...           Show only packages/repos matching <search>
      -e, --exact           Match <search> using plain string
      -s, --show <m,u,s,a>  Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll [default: m]
      -N, --first [<num>]   Show only the first <num> entries
      -n, --last [<num>]    Show only the last <num> entries

Note that `emaint sync` currently [doesn't write to emerge.log](https://bugs.gentoo.org/553788), so
`emlop l --show s` will appear empty if you use `emaint`. Use `emerge --sync` or `eix-sync` instead.

### Estimate how long a merge with take with `predict`

![Predict demo](predict.webp)

Predict-specific arguments:

    Options:
          --tmpdir <dir>       Location of portage tmpdir [default: /var/tmp]
          --resume [<source>]  Use main, backup, any, or no portage resume list
    Filter:
      -s, --show <e,m,t,a>  Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll [default: emt]
      -N, --first [<num>]   Show only the first <num> entries
      -n, --last [<num>]    Show only the last <num> entries
    Stats:
          --limit <num>  Use the last <num> merge times to predict durations [default: 10]
          --avg <fn>     Select function used to predict durations [default: median]

### Show aggregated statistics with `stats`

![Stats demo](stats.webp)

Stats-specific arguments:

    Filter:
      [search]...           Show only packages/repos matching <search>
      -e, --exact           Match <search> using plain string
      -s, --show <p,t,s,a>  Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll [default: p]
    Stats:
      -g, --groupby <y,m,w,d>  Group by (y)ear, (m)onth, (w)eek, or (d)ay
          --limit <num>        Use the last <num> merge times to predict durations [default: 10]
          --avg <fn>           Select function used to predict durations [default: median]

### Other commands

* `complete` generates shell completions
* `accuracy` helps analizing predictions accuracy

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

The current Minimum Supported Rust Version is 1.70. When building `emlop` with an old rustc version,
you might need to pass `--locked` to `cargo install`, to use explicitly tested dependency versions.

#### From crates.io

    cargo install -f emlop

#### From git

    git clone https://github.com/vincentdephily/emlop
    cd emlop
    cargo test
    cargo install -f --path .

#### Shell completion

    emlop complete bash > /usr/share/bash-completion/completions/emlop
    emlop complete zsh > /usr/share/zsh/site-functions/_emlop
    emlop complete fish > /usr/share/fish/vendor_completions.d/emlop.fish

## Contributing

Thanks in advance. See [contributing](docs/CONTRIBUTING.md) for pointers. Emlop is licensed as GPLv3.
