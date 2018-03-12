# EMerge LOg Parser

Emlop parses emerge logs (as generated by [portage](https://wiki.gentoo.org/wiki/Project:Portage),
the [Gentoo](https://www.gentoo.org/) package manager) to yield useful info like merge history and
merge time prediction.

It is heavily inspired by [genlop](https://github.com/gentoo-perl/genlop) but aims to be faster,
more accurate, and more convenient. Other rewrites of Genlop exist, see [COMPARISON](COMPARISON.md)
doc.


## Installation

### From main portage tree

Not available yet, see [gentoo bug 649904](https://bugs.gentoo.org/649904).

### From portage overlay

If you do not have [layman](https://wiki.gentoo.org/wiki/Layman) already, install and configure it.
Then run `layman -a moltonel` to add the overlay with the emlop ebuild. Then run `emerge emlop` as
ususal.

### From source

If you do not have [Rust](https://www.rust-lang.org/) already, install it with `emerge rust` or
[rustup](https://www.rust-lang.org/en-US/install.html). Emlop should always work with the latest
version of rust from portage, but the version from rustup might be more recent and performant.

    git clone https://github.com/vincentdephily/emlop
    cd emlop
    cargo test
    cargo install -f

This installs emlop into `~/.cargo/bin/`, which should be in your `$PATH`. If you wish to install
emlop system-wide, edit the system `$PATH` or copy/symlink `~/.cargo/bin/emlop` somewhere in
`$PATH`.


## Usage

Emlop is split into subcommands like `list` or `predict`, which can be abbreviated by their first
letter. For a complete list of arguments (this readme doesn't list them all) see `emlop -h` or
`emlop <sucommand> -h`.

### Show merge history

Show merge date, merge time, and package name:

    $ emlop list | tail
    2018-01-29 10:20:52 +00:00        13 net-wireless/iw-4.9
    2018-01-29 10:21:21 +00:00        29 dev-libs/librdkafka-0.11.3
    2018-01-29 10:22:27 +00:00      1:06 net-misc/curl-7.58.0
    2018-01-29 11:09:20 +00:00      1:23 media-libs/openexr-2.2.0-r2
    2018-01-29 11:12:18 +00:00      2:58 media-gfx/imagemagick-7.0.7.19
    2018-01-29 11:12:42 +00:00        24 kde-frameworks/kimageformats-5.42.0
    2018-01-29 11:25:32 +00:00     12:50 media-gfx/inkscape-0.92.2
    2018-01-29 12:36:52 +00:00   1:11:20 dev-lang/rust-1.23.0-r1
    2018-01-29 12:37:08 +00:00        16 virtual/rust-1.23.0
    2018-01-29 12:41:54 +00:00      4:46 dev-util/cargo-0.24.0

Same info but filter packages by regexp:

    $ emlop l gcc | tail
    2017-10-04 18:43:31 +01:00         8 sys-devel/gcc-config-1.8-r1
    2017-10-16 13:54:34 +01:00        11 sys-devel/gcc-config-1.8-r1
    2017-10-16 20:00:23 +01:00   1:51:55 sys-devel/gcc-5.4.0-r3
    2017-10-19 11:57:21 +01:00        36 sys-devel/gcc-config-1.8-r1
    2017-11-07 13:06:47 +00:00   1:56:37 sys-devel/gcc-6.4.0
    2017-11-20 12:18:58 +00:00   2:24:20 sys-devel/gcc-6.4.0
    2017-11-20 13:24:59 +00:00        46 sys-devel/gcc-config-1.8-r1
    2017-12-04 18:12:03 +00:00      1:39 sys-devel/gcc-config-1.8-r1
    2017-12-05 12:49:27 +00:00   2:59:33 sys-devel/gcc-6.4.0
    2018-01-12 12:49:17 +00:00   1:48:28 sys-devel/gcc-6.4.0-r1

### Predict merge time

Show currently emerging packages, how long they have been running, and predict how long is left:

    $ emlop p
    Pid 27455: ...n-exec/python3.5/emerge -O chromium         33
    Pid 27848: ...on-exec/python3.5/emerge -O firefox         29
    www-client/firefox                                     53:37 - 24
    www-client/chromium                                  6:01:02 - 28
    Estimate for 2 ebuilds (0 unknown, 52 elapsed)       6:53:47

Predict merge time from an `emerge --pretend` output, taking currently elapsed time into account:

    $ emerge -rOp | emlop p
    Pid 8799: .../emerge -O chromium firefox konqueror   1:14:11
    www-client/chromium                                  5:49:38 - 1:10:55
    www-client/firefox                                     53:37
    kde-apps/konqueror                                      3:46
    Estimate for 3 ebuilds (0 unknown, 1:10:55 elapsed)  5:36:06

### Show merge statistics

Show total merge time, merge count, and average merge time:

    $ emlop s gtk
    app-admin/gtkdiskfree                1:19    1      1:19
    net-libs/webkit-gtk              63:17:43   44   1:57:33
    x11-libs/gtk+                     3:10:20   40      4:21
    x11-themes/gtk-engines-adwaita       1:23    4        20
    dev-util/gtk-doc                     4:46    9        31
    dev-python/pygtk                    16:05    7      2:17
    dev-util/gtk-doc-am                  3:43   19        11
    x11-libs/gtksourceview               4:54    6        49
    dev-python/pygtksourceview           2:27    6        24
    dev-perl/gtk2-ex-formfactory         2:29   10        14
    dev-util/gtk-update-icon-cache       5:44   16        23
    dev-cpp/gtkmm                       39:33   12      3:40
    dev-python/pywebkitgtk                 13    1        13
    dev-perl/gtk2-perl                  12:49    8      1:36


## Contributing

Thanks, and welcome :) See [CONTRIBUTING](CONTRIBUTING.md). Emlop is licensed as GPLv3.
