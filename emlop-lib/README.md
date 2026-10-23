# Emerge and portage parsing library

Emlop-lib provides information about past and present
[Portage](https://wiki.gentoo.org/wiki/Project:Portage) activity:

* Emerge log history
* Current merge list (`mtime.db`, `emerge -pv`)
* Package renames
* Emerge processes
* Build logs

## Usage

    cargo add emlop-lib

For more information, see

* Online [API docs](https://docs.rs/crate/emlop-lib/latest)
* Code [examples](examples)
* [Emlop](https://github.com/vincentdephily/emlop) end-user CLI
