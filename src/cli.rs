use clap::{crate_version, App, AppSettings, Arg, SubCommand};

/// Generate cli argument parser without the `complete` subcommand.
pub fn build_cli_nocomplete() -> App<'static, 'static> {
    let arg_limit =
        Arg::with_name("limit").long("limit")
                               .takes_value(true)
                               .default_value("10")
                               .help("Use the last N merge times to predict next merge time.");
    let arg_pkg =
        Arg::with_name("package").takes_value(true).help("Show only packages matching <package>.");
    let arg_exact = Arg::with_name("exact")
        .short("e")
        .long("exact")
        .help("Match package with a string instead of a regex.")
        .long_help("Match package with a string instead of a regex. \
Regex is case-insensitive and matches on category/name (see https://docs.rs/regex/1.1.0/regex/#syntax). \
String is case-sentitive and matches on whole name, or whole category/name if it contains a /."); //FIXME auto crate version
    let arg_show_l = Arg::with_name("show")
        .short("s")
        .long("show")
        .value_name("m,u,s,a")
        .validator(|s| find_invalid("musa", &s))
        .default_value("m")
        .help("Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll.")
        .long_help("Show individual (m)erges, (u)nmerges, portage tree (s)yncs, or (a)ll of these (any letters combination).");
    let arg_show_s = Arg::with_name("show")
        .short("s")
        .long("show")
        .value_name("p,t,s,a")
        .validator(|s| find_invalid("ptsa", &s))
        .default_value("p")
        .help("Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll.")
        .long_help("Show per-(p)ackage merges/unmerges, (t)otal merges/unmerges, portage tree (s)yncs, or (a)ll of these (any letters combination).");
    let arg_group = Arg::with_name("group")
        .short("g")
        .long("groupby")
        .value_name("y,m,w,d")
        .possible_values(&["y","m","w","d"])
        .hide_possible_values(true)
        .help("Group by (y)ear, (m)onth, (w)eek, or (d)ay.")
        .long_help("Group by (y)ear, (m)onth, (w)eek, or (d)ay.\n\
The grouping key is displayed in the first column. Weeks start on monday and are formated as 'year-weeknumber'.");
    App::new("emlop")
        .version(crate_version!())
        .global_setting(AppSettings::ColoredHelp)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .global_setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::VersionlessSubcommands)
        .about("A fast, accurate, ergonomic EMerge LOg Parser.\nhttps://github.com/vincentdephily/emlop")
        .after_help("Subcommands can be abbreviated down to a single letter.\n\
Exit code is 0 if sucessful, 1 in case of errors (bad argument...), 2 if search found nothing.")
        .help_message("Show short (-h) or detailed (--help) help. Use <subcommand> -h/--help for subcommand help.")
        .arg(Arg::with_name("utc")
             .long("utc")
             .global(true)
             .help("Parse naive dates as UTC instead of local time"))
        .arg(Arg::with_name("from")
             .value_name("date")
             .short("f")
             .long("from")
             .global(true)
             .takes_value(true)
             .help("Only parse log entries after <date>.")
             .long_help("Only parse log entries after <date>.\n\
Accepts formats like '2018-03-04', '2018-03-04 12:34:56', '2018-03-04T12:34', '1 year, 2 months', '10d', and unix timestamps."))
        .arg(Arg::with_name("to")
             .value_name("date")
             .short("t")
             .long("to")
             .global(true)
             .takes_value(true)
             .help("Only parse log entries before <date>."))
        .arg(Arg::with_name("duration")
             .value_name("hms,s")
             .long("duration")
             .global(true)
             .possible_values(&["hms","s"])
             .hide_possible_values(true)
             .default_value("hms")
             .help("Output durations in hours:minutes:seconds or in seconds."))
        .arg(Arg::with_name("date")
             .value_name("format")
             .long("date")
             .global(true)
             .possible_values(&["ymd","d","ymdhms","dt","ymdhmso","dto","rfc3339","3339","rfc2822","2822","compact","unix"])
             .hide_possible_values(true)
             .default_value("ymdhms")
             .help("Output dates in different formats (get the list with `--date help`)."))
        .arg(Arg::with_name("logfile")
             .value_name("file")
             .long("logfile")
             .short("F")
             .global(true)
             .takes_value(true)
             .default_value("/var/log/emerge.log")
             .help("Location of emerge log file."))
        .arg(Arg::with_name("verbose")
             .short("v")
             .global(true)
             .multiple(true)
             .help("Show warnings (-v), info (-vv) and debug (-vvv) messages (errors are always displayed)."))
        .arg(Arg::with_name("color")
             .long("color").alias("colour")
             .global(true)
             .takes_value(true)
             .possible_values(&["auto","always","never","y","n"])
             .hide_possible_values(true)
             .default_value("auto")
             .value_name("when")
             .help("Enable color (auto/always/never/y/n)."))
        .subcommand(SubCommand::with_name("log")
                    .about("Show log of sucessful merges, unmerges and syncs.")
                    .long_about("Show log of sucessful merges, unmerges and syncs.\n\
* (Un)merges: date, duration, package name-version.\n\
* Syncs:      date, duration.")
                    .help_message("Show short (-h) or detailed (--help) help.")
                    .arg(&arg_show_l)
                    .arg(&arg_exact)
                    .arg(&arg_pkg))
        .subcommand(SubCommand::with_name("predict")
                    .about("Predict merge time for current or pretended merges.")
                    .long_about("Predict merge time for current or pretended merges.\n\
* If input is a terminal, predict time for the current merge (if any).\n\
* If input is a pipe (for example by running `emerge -rOp|emlop p`), predict time for those merges.")
                    .help_message("Show short (-h) or detailed (--help) help.")
                    .arg(&arg_limit))
        .subcommand(SubCommand::with_name("stats")
                    .about("Show statistics about sucessful merges, unmerges and syncs.")
                    .long_about("Show statistics about sucessful (un)merges (overall or per package) and syncs.\n\
* <package>: merge count, total merge time, predicted merge time, unmerge count, total unmerge time, predicted unmerge time.\n\
* Total:     merge count, total merge time, average merge time,   unmerge count, total unmerge time, average unmerge time.\n\
* Sync:      sync count,  total sync time,  predicted sync time.")
                    .help_message("Show short (-h) or detailed (--help) help.")
                    .arg(&arg_show_s)
                    .arg(&arg_group)
                    .arg(&arg_exact)
                    .arg(&arg_pkg)
                    .arg(&arg_limit))
}

/// Generate cli argument parser.
pub fn build_cli() -> App<'static, 'static> {
    let c = build_cli_nocomplete();
    c.subcommand(SubCommand::with_name("complete")
                 .about("Generate shell completion script.")
                 .long_about("Write shell completion script to stdout.\n\n\
You should redirect the output to a file that will be sourced by your shell.\n\
For example: `emlop complete bash > ~/.bash_completion.d/emlop`.\n\
To apply the changes, either restart you shell or `source` the generated file.
")
                 .arg(Arg::with_name("shell")
                      .help("Target shell")
                      .required(true)
                      .possible_values(&["bash","zsh","fish"])))
}

/// Clap validation helper that checks that all chars are valid.
fn find_invalid(valid: &'static str, s: &str) -> Result<(), String> {
    debug_assert!(valid.is_ascii()); // Because we use `chars()` we need to stick to ascii for `valid`.
    match s.chars().find(|&c| !(valid.contains(c))) {
        None => Ok(()),
        Some(p) => Err(p.to_string()),
    }
}
