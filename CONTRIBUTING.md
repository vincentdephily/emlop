# Contributing to emlop

Thank you for taking the time to contribute.

Follow the [Rust](https://www.rust-lang.org/en-US/conduct.html) and
[Gentoo](https://wiki.gentoo.org/wiki/Project:Council/Code_of_conduct) codes of conduct.

## Reporting bugs and feature requests

Please create issues via [Github](https://github.com/vincentdephily/emlop/issues). You might want to
peek at the [roadmap](ROADMAP.md) for inspiration.

## Submitting patches

Send pull requests to [Github](https://github.com/vincentdephily/emlop).

Make sure to `cargo test -- --include-ignored` before submitting your code. A bugfix should probably
include an updated unittest. Check `cargo clippy` hints. Format code using `cargo +nightly fmt`.

Test emlop with the latest rust stable versions from both Gentoo and upstream. Using `rustup` is
highly recomended.

Speed is important, check for improvements/regressions using `benches/exec_compare.rs` (you need to
`cargo install scriptisto` to be able to run this file).

Respect [semver](https://semver.org/).

## Status on other environements

I only have access to amd64/linux/gentoo/portage environements. Reports about running emlop on
arm/freebsd/funtoo/paludis/etc would be appreciated.

## License

Emlop is licensed as GPLv3. Any contribution accepted into the emlop repo will have that license,
unless the contributor explicitly demands otherwise.

## Release checklist

* Update deps: `cargo outdated`, edit Cargo.toml, `cargo update`.
* Check `git status` and either `commit` or `stash`.
* Test: `rustup override set 1.58.1 && cargo test -- --include-ignored && rustup override unset && cargo test -- --include-ignored`.
* Update/commit CHANGELOG.md, Cargo.toml, Cargo.lock.
* `git tag <version> -a` (copy the changelog entry into the tag).
* `git push --tags`.
* Create new ebuild in [moltonel-ebuilds](https://github.com/vincentdephily/moltonel-ebuilds).
* Publish to [crates.io](https://crates.io/).
* Send a [bump request](https://bugs.gentoo.org/).
