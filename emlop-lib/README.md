# Emerge and portage log/files/process parsing library

Emlop-lib provides information about past and present activity of
[Portage](https://wiki.gentoo.org/wiki/Project:Portage), [Gentoo](https://gentoo.org)'s package
manager:

* Emerge log history (merges, unmerge, syncs, runs...)
* Current merge list (from `mtime.db` or `emerge -pv`)
* Package renames
* Emerge process tree
* Build logs

## Usage

    cargo add emlop-lib

For more information, see

* Online [API docs](https://docs.rs/emlop-lib/latest)
* Code [examples](examples)
* [Emlop](https://github.com/vincentdephily/emlop/emlop) end-user CLI
