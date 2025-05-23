# Contributing to emlop

Thank you for taking the time to contribute.

Follow the [Rust](https://www.rust-lang.org/en-US/conduct.html) and
[Gentoo](https://wiki.gentoo.org/wiki/Project:Council/Code_of_conduct) codes of conduct.

## Reporting bugs or feature requests

Please create issues via [Github](https://github.com/vincentdephily/emlop/issues). Check existing
issues, and make sure you're running the latest version.

## Sending patches

Emlop is licensed as GPLv3, any contribution accepted into the emlop repo will have that license.
Send pull requests via [Github](https://github.com/vincentdephily/emlop). Using AI is ok but must be
mentioned in the PR.

Run `cargo test -- --include-ignored` before submitting your code. A bugfix should probably
include a new/updated unittest. Check `cargo clippy` hints. Format code using `cargo +nightly fmt`. The
github CI also runs these checks.

Test emlop with the latest rust stable versions from both Gentoo and upstream, and the oldest
version from Gentoo. Using `rustup` is highly recomended.

Check for performance improvements/regressions using `benches/exec_compare.rs` (you need to
`cargo install scriptisto` to be able to run this file) and `cargo +nightly bench -F unstable bench`
(you need a nightly toolchain installed).

I only have access to amd64/linux/gentoo/portage environements. Reports about running emlop on
arm/freebsd/funtoo/paludis/etc would be appreciated.

## Release checklist

* Update deps:
  - Enable [MSRV-aware resolver](https://doc.rust-lang.org/cargo/reference/config.html#resolverincompatible-rust-versions)
  - `cargo outdated`, check changelogs, edit Cargo.toml if needed
  - `cargo update; cargo test`
* Check `git status` and either `commit+push` or `stash`.
* Check github CI status.
* Update/commit CHANGELOG.md, Cargo.toml, Cargo.lock.
* `git tag <version> -a` (copy the changelog entry into the tag).
* `git push --tags`.
* Create release from tag in github
* Create new ebuild in [moltonel-ebuilds](https://github.com/vincentdephily/moltonel-ebuilds).
  - Use `cargo package` to get a crate to test
  - Check against main repo ebuild
  - Check tests and useflag variations
* Publish to [crates.io](https://crates.io/).
* Send a [pull request](https://github.com/gentoo/gentoo/pulls).
