use clap::{builder::styling, crate_version, value_parser, Arg, ArgAction::*, Command};
use std::path::PathBuf;

/// Generate cli argument parser without the `complete` subcommand.
pub fn build_cli_nocomplete() -> Command {
    ////////////////////////////////////////////////////////////
    // Filter arguments
    ////////////////////////////////////////////////////////////
    let pkg = Arg::new("search").num_args(..)
                                .display_order(1)
                                .help_heading("Filter")
                                // Workaround bad alignment, might be fixed in clap 4
                                .help("    Show only packages/repos matching <search>")
                                .long_help("Show only packages/repos matching <search>\n\
                                            Multiple terms can be provided\n\
                                            Matches using a regex unless `--exact` is specified\n\
                                            See https://docs.rs/regex/*/regex/#syntax\n  \
                                            rust:        Matches `dev-lang/rust`, `dev-util/rustup`, `dev-python/trustme`, etc\n  \
                                            /[pc]ython$: Matches `dev-lang/python` and `dev-python/cython`\n  \
                                            pyqt:        Matches `dev-python/PyQt5` (case-insensitive)\n  \
                                            guru:        Matches `guru` (repo sync)");
    let exact = Arg::new("exact").short('e')
                                 .long("exact")
                                 .action(SetTrue)
                                 .display_order(2)
                                 .help_heading("Filter")
                                 .help("Match <search> using plain string")
                                 .long_help("Match <search> using plain string\n  \
                                             rust:         Matches both `dev-lang/rust` and `virtual/rust`\n  \
                                             virtual/rust: Matches only `virtual/rust`\n  \
                                             RuSt:         Matches nothing (case-sensitive)\n  \
                                             ru:           Matches nothing (whole name only)");

    let show_l = Arg::new("show").short('s')
                                 .long("show")
                                 .value_name("m,u,s,a")
                                 .display_order(3)
                                 .help_heading("Filter")
                                 .help("Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll")
                                 .long_help("Show (any combination of)\n  \
                                             m: Package merges\n  \
                                             u: Package unmerges\n  \
                                             s: Repository syncs\n  \
                                             a: All of the above");
    let show_s = Arg::new("show").short('s')
                                 .long("show")
                                 .value_name("p,t,s,a")
                                 .display_order(3)
                                 .help_heading("Filter")
                                 .help("Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll")
                                 .long_help("Show (any combination of)\n  \
                                             p: Individual package merges/unmerges\n  \
                                             t: Total package merges/unmerges\n  \
                                             s: Repository syncs\n  \
                                             a: All of the above");
    let show_p = Arg::new("show").short('s')
                                 .long("show")
                                 .value_name("e,m,t,a")
                                 .display_order(3)
                                 .help_heading("Filter")
                                 .help("Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll")
                                 .long_help("Show (any combination of)\n  \
                                             e: Current emerge processes\n  \
                                             m: Package merges\n  \
                                             t: Total estimate\n  \
                                             a: All of the above");
    let show_a = Arg::new("show").short('s')
                                 .long("show")
                                 .value_name("m,t,a")
                                 .display_order(3)
                                 .help_heading("Filter")
                                 .help("Show (m)erges, (t)otals, and/or (a)ll")
                                 .long_help("Show (any combination of)\n  \
                                             m: Package merges\n  \
                                             t: Totals\n  \
                                             a: All of the above");

    let from = Arg::new("from").value_name("date")
                               .short('f')
                               .long("from")
                               .display_order(4)
                               .global(true)
                               .num_args(1)
                               .help_heading("Filter")
                               .help("Only parse log entries after <date>")
                               .long_help("Only parse log entries after <date>\n  \
                                           2018-03-04|2018-03-04 12:34:56|2018-03-04T12:34: Absolute ISO date\n  \
                                           123456789:                                       Absolute unix timestamp\n  \
                                           1 year, 2 months|10d:                            Relative date");
    let to = Arg::new("to").value_name("date")
                           .short('t')
                           .long("to")
                           .display_order(5)
                           .global(true)
                           .num_args(1)
                           .help_heading("Filter")
                           .help("Only parse log entries before <date>")
                           .long_help("Only parse log entries before <date>\n  \
                                       2018-03-04|2018-03-04 12:34:56|2018-03-04T12:34: Absolute ISO date\n  \
                                       123456789:                                       Absolute unix timestamp\n  \
                                       1 year, 2 months|10d:                            Relative date");
    let first = Arg::new("first").short('N')
                                 .long("first")
                                 .value_name("num")
                                 .display_order(6)
                                 .num_args(..=1)
                                 .default_missing_value("1")
                                 .value_parser(value_parser!(usize))
                                 .help_heading("Filter")
                                 .help("Show only the first <num> entries")
                                 .long_help("Show only the first <num> entries\n  \
                                             (empty)|1: first entry\n  \
                                             5:         first 5 entries\n");
    let last = Arg::new("last").short('n')
                               .long("last")
                               .value_name("num")
                               .display_order(7)
                               .num_args(..=1)
                               .default_missing_value("1")
                               .value_parser(value_parser!(usize))
                               .help_heading("Filter")
                               .help("Show only the last <num> entries")
                               .long_help("Show only the last <num> entries\n  \
                                             (empty)|1: last entry\n  \
                                             5:         last 5 entries\n");

    ////////////////////////////////////////////////////////////
    // Stats arguments
    ////////////////////////////////////////////////////////////
    let group = Arg::new("group").short('g')
                                 .long("groupby")
                                 .display_order(1)
                                 .value_name("y,m,w,d,n")
                                 .hide_possible_values(true)
                                 .help_heading("Stats")
                                 .help("Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one")
                                 .long_help("Group by (y)ear, (m)onth, (w)eek, (d)ay, or (n)one\n\
                                             The grouping key is displayed in the first column.\n\
                                             Weeks start on monday and are formated as \
                                             'year-weeknumber'.");
    let limit = Arg::new("limit").long("limit")
                                 .display_order(2)
                                 .num_args(1)
                                 .value_name("num")
                                 .help_heading("Stats")
                                 .help("Use the last <num> merge times to predict durations");
    let avg =
        Arg::new("avg").long("avg")
                       .value_name("fn")
                       .display_order(3)
                       .hide_possible_values(true)
                       .help_heading("Stats")
                       .help("Select function used to predict durations")
                       .long_help("Select function used to predict durations\n  \
                                   arith|a:            simple 'sum/count' average\n  \
                                   (defaut)|median|m:  middle value, mitigates outliers\n  \
                                   weighted-arith|wa:  'sum/count' with more weight for recent values\n  \
                                   weighted-median|wm: \"middle\" value shifted toward recent values");

    let unknown = Arg::new("unknown").long("unknown")
                                     .display_order(4)
                                     .num_args(1)
                                     .value_name("secs")
                                     .help_heading("Stats")
                                     .help("Assume unkown packages take <secs> seconds to merge");

    ////////////////////////////////////////////////////////////
    // Format arguments
    ////////////////////////////////////////////////////////////
    let header = Arg::new("header").short('H')
                                   .long("header")
                                   .global(true)
                                   .display_order(1)
                                   .num_args(..=1)
                                   .default_missing_value("y")
                                   .value_name("bool")
                                   .help_heading("Format")
                                   .help("Show table header");
    let date =
        Arg::new("date").value_name("format")
                        .long("date")
                        .display_order(2)
                        .global(true)
                        .display_order(52)
                        .help_heading("Format")
                        .help("Output dates in different formats")
                        .long_help("Output dates in different formats\n  \
                                    ymd|d:               2022-01-31\n  \
                                    (default)|ymdhms|dt: 2022-01-31 08:59:46\n  \
                                    ymdhmso|dto:         2022-01-31 08:59:46 +00:00\n  \
                                    rfc3339|3339:        2022-01-31T08:59:46+00:00\n  \
                                    rfc2822|2822:        Mon, 31 Jan 2022 08:59:46 +00:00\n  \
                                    compact:             20220131085946\n  \
                                    unix:                1643619586");
    let duration = Arg::new("duration").value_name("format")
                                       .long("duration")
                                       .display_order(3)
                                       .global(true)
                                       .hide_possible_values(true)
                                       .display_order(51)
                                       .help_heading("Format")
                                       .help("Output durations in different formats")
                                       .long_help("Output durations in different formats\n  \
                                                   hms|(default): 10:30\n  \
                                                   hmsfixed:      0:10:30\n  \
                                                   secs|s:        630\n  \
                                                   human|h:       10 minutes, 30 seconds");
    let utc = Arg::new("utc").long("utc")
                             .global(true)
                             .display_order(4)
                             .num_args(..=1)
                             .default_missing_value("y")
                             .value_name("bool")
                             .help_heading("Format")
                             .help("Parse/display dates in UTC instead of local time");
    let starttime = Arg::new("starttime").long("starttime")
                                         .num_args(..=1)
                                         .default_missing_value("y")
                                         .value_name("bool")
                                         .display_order(5)
                                         .help_heading("Format")
                                         .help("Display start time instead of end time");
    let color = Arg::new("color").long("color")
                                 .alias("colour")
                                 .display_order(6)
                                 .global(true)
                                 .value_parser(value_parser!(crate::ColorStyle))
                                 .hide_possible_values(true)
                                 .num_args(..=1)
                                 .default_missing_value("y")
                                 .value_name("when")
                                 .display_order(55)
                                 .help_heading("Format")
                                 .help("Enable color (always/never/y/n)")
                                 .long_help("Enable color (always/never/y/n)\n  \
                                             (default):        colored if on tty\n  \
                                             (empty)|always|y: colored\n  \
                                             never|n:          not colored");
    let output = Arg::new("output").long("output")
                                   .short('o')
                                   .value_name("format")
                                   .global(true)
                                   .value_parser(value_parser!(crate::OutStyle))
                                   .hide_possible_values(true)
                                   .display_order(7)
                                   .help_heading("Format")
                                   .help("Ouput format (columns/c/tab/t)")
                                   .long_help("Ouput format (columns/c/tab/t)\n  \
                                               (default): columns on tty, tab otherwise\n  \
                                               columns|c: space-aligned columns\n  \
                                               tab|t:     tab-separated values");

    ////////////////////////////////////////////////////////////
    // Misc arguments
    ////////////////////////////////////////////////////////////
    let logfile = Arg::new("logfile").value_name("file")
                                     .long("logfile")
                                     .short('F')
                                     .global(true)
                                     .num_args(1)
                                     .display_order(1)
                                     .help("Location of emerge log file");
    let tmpdir = Arg::new("tmpdir").value_name("dir")
                                   .long("tmpdir")
                                   .num_args(1)
                                   .action(Append)
                                   .value_parser(value_parser!(PathBuf))
                                   .display_order(2)
                                   .help("Location of portage tmpdir")
                                   .long_help("Location of portage tmpdir\n\
                                               Multiple folders can be provided\n\
                                               Emlop also looks for tmpdir using current emerge processes");
    let h = "Use main, backup, any, or no portage resume list\n\
             This is ignored if STDIN is a piped `emerge -p` output\n  \
             (default):     Use main resume list, if currently emerging\n  \
             any|a|(empty): Use main or backup resume list\n  \
             main|m:        Use main resume list\n  \
             backup|b:      Use backup resume list\n  \
             no|n:          Never use resume list";
    let resume = Arg::new("resume").long("resume")
                                   .value_name("source")
                                   .value_parser(value_parser!(crate::config::ResumeKind))
                                   .hide_possible_values(true)
                                   .num_args(..=1)
                                   .default_missing_value("any")
                                   .display_order(3)
                                   .help(h.split_once('\n').unwrap().0)
                                   .long_help(h);
    let verbose = Arg::new("verbose").short('v')
                                     .global(true)
                                     .action(Count)
                                     .display_order(4)
                                     .help("Increase verbosity (can be given multiple times)")
                                     .long_help("Increase verbosity (defaults to errors only)\n  \
                                                 -v:   show warnings\n  \
                                                 -vv:  show info\n  \
                                                 -vvv: show debug");
    let h = "Location of emlop config file\n\
             Default is $HOME/.config/emlop.toml (or $EMLOP_CONFIG if set)\n\
             Set to an empty string to disable\n\
             Config in in TOML format, see example file in /usr/share/doc/emlop-x.y.z/";
    let config = Arg::new("config").value_name("file")
                                   .long("config")
                                   .global(true)
                                   .num_args(1)
                                   .display_order(5)
                                   .help(h.split_once('\n').unwrap().0)
                                   .long_help(h);

    ////////////////////////////////////////////////////////////
    // Subcommands
    ////////////////////////////////////////////////////////////
    let h = "Show log of sucessful merges, unmerges and syncs\n\
             * (Un)merges: date, duration, package name-version\n\
             * Syncs:      date, duration, repository";
    let cmd_log = Command::new("log").about(h.split_once('\n').unwrap().0)
                                     .long_about(h)
                                     .arg(starttime)
                                     .arg(&first)
                                     .arg(&last)
                                     .arg(show_l)
                                     .arg(&exact)
                                     .arg(&pkg);
    let h = "Predict merge times for current or pretended merges\n\
             * If input is a terminal, predict times for the current merges (if any)\n\
             * If input is a pipe (for example by running `emerge -rOp|emlop p`), \
             predict times for those merges.";
    let cmd_pred = Command::new("predict").about(h.split_once('\n').unwrap().0)
                                          .long_about(h)
                                          .arg(show_p)
                                          .arg(first)
                                          .arg(&last)
                                          .arg(tmpdir)
                                          .arg(resume)
                                          .arg(unknown)
                                          .arg(&avg)
                                          .arg(&limit);
    let h = "Show statistics about syncs, per-package (un)merges, and total (un)merges\n\
             * Sync:      count,       total time, predicted time\n\
             * <package>: merge count, total time, predicted time, unmerge count, total time, predicted time\n\
             * Total:     merge count, total time, average time,   unmerge count, total time, average time";
    let cmd_stats = Command::new("stats").about(h.split_once('\n').unwrap().0)
                                         .long_about(h)
                                         .arg(show_s)
                                         .arg(group)
                                         .arg(&exact)
                                         .arg(&pkg)
                                         .arg(&avg)
                                         .arg(&limit);
    let h = "Compare actual merge time against predicted merge time\n\
             Use this to gauge the effect of the --limit and --avg options";
    let cmd_accuracy = Command::new("accuracy").about(h.split_once('\n').unwrap().0)
                                               .long_about(h)
                                               .arg(pkg)
                                               .arg(exact)
                                               .arg(show_a)
                                               .arg(last)
                                               .arg(avg)
                                               .arg(limit);

    ////////////////////////////////////////////////////////////
    // Main command
    ////////////////////////////////////////////////////////////
    let about = "A fast, accurate, ergonomic EMerge LOg Parser\n\
                 https://github.com/vincentdephily/emlop";
    let after_help =
        "Subcommands and long args can be abbreviated (eg `emlop l -ss --head -f1w`)\n\
         Subcommands have their own -h / --help\n\
         Exit code is 0 if sucessful, 1 if search found nothing, 2 in case of other errors";
    let styles =
        styling::Styles::styled().header(styling::AnsiColor::Blue.on_default()
                                         | styling::Effects::BOLD)
                                 .usage(styling::AnsiColor::Blue.on_default()
                                        | styling::Effects::BOLD)
                                 .literal(styling::AnsiColor::Green.on_default())
                                 .placeholder(styling::AnsiColor::Cyan.on_default());
    Command::new("emlop").version(crate_version!())
                         .disable_help_subcommand(true)
                         .infer_subcommands(true)
                         .infer_long_args(true)
                         .arg_required_else_help(true)
                         .styles(styles)
                         .subcommand_required(true)
                         .about(about)
                         .after_help(after_help)
                         .arg(from)
                         .arg(to)
                         .arg(header)
                         .arg(duration)
                         .arg(date)
                         .arg(utc)
                         .arg(color)
                         .arg(output)
                         .arg(logfile)
                         .arg(config)
                         .arg(verbose)
                         .subcommand(cmd_log)
                         .subcommand(cmd_pred)
                         .subcommand(cmd_stats)
                         .subcommand(cmd_accuracy)
}

/// Generate cli argument parser.
pub fn build_cli() -> Command {
    let labout = "Write shell completion script to stdout\n\n\
                  You should redirect the output to a file that will be sourced by your shell\n\
                  For example: `emlop complete bash > ~/.bash_completion.d/emlop`\n\
                  To apply the changes, either restart you shell or `source` the generated file";
    let shell = Arg::new("shell").help("Target shell")
                                 .required(true)
                                 .value_parser(value_parser!(clap_complete::Shell));
    let cmd = Command::new("complete").about("Generate shell completion script")
                                      .long_about(labout)
                                      .arg(shell);
    build_cli_nocomplete().subcommand(cmd)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;

    fn matches(args: &str) -> clap::ArgMatches {
        build_cli().get_matches_from(format!("emlop {args}").split_whitespace())
                   .subcommand()
                   .expect(&format!("Failed parsing {args:?}"))
                   .1
                   .clone()
    }

    macro_rules! one {
        ($t: ty, $k: expr, $a: expr) => {
            matches($a).get_one::<$t>($k)
        };
    }
    macro_rules! many {
        ($t: ty, $k: expr, $a: expr) => {
            matches($a).get_many::<$t>($k).map(|r| r.cloned().collect::<Vec<$t>>())
        };
    }

    #[test]
    fn args() {
        assert_eq!(one!(usize, "first", "l"), None);
        assert_eq!(one!(usize, "first", "l --first"), Some(&1usize));
        assert_eq!(one!(usize, "first", "l --first"), Some(&1usize));
        assert_eq!(one!(usize, "first", "l --first 2"), Some(&2usize));
        assert_eq!(one!(usize, "first", "l -N 2"), Some(&2usize));
        assert_eq!(one!(usize, "first", "l -N4"), Some(&4usize));

        assert_eq!(one!(usize, "last", "l --last"), Some(&1usize));
        assert_eq!(one!(usize, "last", "l --last 2"), Some(&2usize));
        assert_eq!(one!(usize, "last", "l -n 2"), Some(&2usize));

        assert_eq!(one!(ColorStyle, "color", "l"), None);
        assert_eq!(one!(ColorStyle, "color", "l --color"), Some(&ColorStyle::Always));
        assert_eq!(one!(ColorStyle, "color", "l --color=y"), Some(&ColorStyle::Always));
        assert_eq!(one!(ColorStyle, "color", "l --color n"), Some(&ColorStyle::Never));
        assert_eq!(one!(ColorStyle, "color", "l --color never"), Some(&ColorStyle::Never));

        let pathvec = |s: &str| Some(s.split_whitespace().map(PathBuf::from).collect());
        assert_eq!(many!(PathBuf, "tmpdir", "p --tmpdir a"), pathvec("a"));
        assert_eq!(many!(PathBuf, "tmpdir", "p --tmpdir a --tmpdir b"), pathvec("a b"));
    }
}