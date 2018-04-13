# Contributing to emlop

Thanks for your interest in emlop.
We welcome bug reports, code patches, and friendly encouragements :)

Be nice and considerate to other users and contributors.

## Reporting bugs and feature requests

Please create issues via [Github](https://github.com/vincentdephily/emlop/issues). Check the
[roadmap](ROADMAP.md) before opening an issue for a new feature, unless you have more insight and/or
help to offer.

## Submitting patches

Send pull requests to [Github](https://github.com/vincentdephily/emlop).

Make sure to `cargo test` before submitting your code. A bugfix should probably include an updated
unittest.

Test emlop with both the latest rust version available on Gentoo and the latest upstream stable rust
version. Using `rustup` is highly recomended.

`rustfmt` and `clippy` are also recomended, but not followed religiously.

Speed is important, check for improvements/regressions using `benches/exec_compare.crs` (you need to
`cargo install script` to be able to run this file).

Respect [semver](https://semver.org/).

## License

Emlop is licensed as GPLv3. Any contribution accepted into the emlop repo will have that license,
unless the contributor explicitly demands otherwise.

## Release checklist

* Update deps: `cargo outdated`, edit Cargo.toml, `cargo update`.
* Check `git status` and either `commit` or `stash`.
* Test: `rustup override set 1.23.0 && cargo test && rustup override unset && cargo test`.
* Update/commit CHANGELOG.md and Cargo.toml.
* `git tag <version> -a` (copy the changelog entry into the tag).
* `git push --tags`.
* Create new ebuild in [moltonel-ebuilds](https://github.com/vincentdephily/moltonel-ebuilds).
* Send a [bump request](https://bugs.gentoo.org).
