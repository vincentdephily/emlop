use ::*;
use parser::*;
use proces::*;

use std::collections::{BTreeMap, HashMap};
use std::io::stdin;

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_list(args: &ArgMatches, subargs: &ArgMatches) -> Result<bool, Error> {
    let hist = Parser::new_hist(myopen(args.value_of("logfile").unwrap())?, args.value_of("logfile").unwrap(),
                                value_opt(args, "from", parse_date), value_opt(args, "to", parse_date),
                                subargs.value_of("package"), subargs.is_present("exact"))?;
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut found_one = false;
    for p in hist {
        match p {
            Parsed::Start{ts, ebuild, version, iter, ..} => {
                // This'll overwrite any previous entry, if a build started but never finished
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            Parsed::Stop{ts, ebuild, version, iter, ..} => {
                found_one = true;
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(prevts) => writeln!(io::stdout(), "{} {:>9} {}-{}",     fmt_time(ts), fmt_duration(ts - prevts), ebuild, version).unwrap_or(()),
                    None =>         writeln!(io::stdout(), "{}  00:00:00 {}-{}", fmt_time(ts), ebuild, version).unwrap_or(()),
                }
            },
            _ => assert!(false, "unexpected {:?}", p),
        }
    }
    Ok(found_one)
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
pub fn cmd_stats(tw: &mut TabWriter<io::Stdout>, args: &ArgMatches, subargs: &ArgMatches) -> Result<bool, Error> {
    let hist = Parser::new_hist(myopen(args.value_of("logfile").unwrap())?, args.value_of("logfile").unwrap(),
                                value_opt(args, "from", parse_date), value_opt(args, "to", parse_date),
                                subargs.value_of("package"), subargs.is_present("exact"))?;
    let lim = value(subargs, "limit", parse_limit);
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut times: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut found_one = false;
    for p in hist {
        match p {
            Parsed::Start{ts, ebuild, version, iter, ..} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            Parsed::Stop{ts, ebuild, version, iter, ..} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(start_ts) => {
                        let timevec = times.entry(ebuild.clone()).or_insert(vec![]);
                        timevec.insert(0, ts-start_ts);
                    },
                    None => (),
                }
            },
            _ => assert!(false, "unexpected {:?}", p),
        }
    };
    for (pkg,tv) in times.iter() {
        found_one = true;
        let (predtime,predcount,tottime,totcount) = tv.iter()
            .fold((0,0,0,0), |(pt,pc,tt,tc), &i| {
                if tc >= lim {(pt,  pc,  tt+i,tc+1)}
                else         {(pt+i,pc+1,tt+i,tc+1)}
            });
        writeln!(tw, "{}\t{:>9}\t{:>3}\t{:>8}", pkg, fmt_duration(tottime), totcount, fmt_duration(predtime/predcount))?;
    }
    Ok(found_one)
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(tw: &mut TabWriter<io::Stdout>, args: &ArgMatches, subargs: &ArgMatches) -> Result<bool, Error> {
    let now = epoch_now();
    let lim = value(subargs, "limit", parse_limit);

    // Gather and print info about current merge process.
    let mut cms = std::i64::MAX;
    for i in get_all_info(Some("emerge"))? {
        cms = std::cmp::min(cms, i.start);
        writeln!(tw, "Pid {}: ...{}\t{:>9}", i.pid, &i.cmdline[(i.cmdline.len()-35)..], fmt_duration(now-i.start))?;
    }
    if cms == std::i64::MAX && atty::is(atty::Stream::Stdin) {
        writeln!(tw, "No ongoing merge found")?;
        return Ok(false)
    }

    // Parse emerge log.
    let hist = Parser::new_hist(myopen(args.value_of("logfile").unwrap())?, args.value_of("logfile").unwrap(),
                                value_opt(args, "from", parse_date), value_opt(args, "to", parse_date),
                                None, false)?;
    let mut started: HashMap<(String, String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    for p in hist {
        match p {
            // We're ignoring iter here (reducing the start->stop matching accuracy) because there's no iter in the pretend output.
            Parsed::Start{ts, ebuild, version, ..} => {
                started.insert((ebuild.clone(), version.clone()), ts);
            }
            Parsed::Stop{ts, ebuild, version, ..} => {
                if let Some(start_ts) = started.remove(&(ebuild.clone(), version.clone())) {
                    let timevec = times.entry(ebuild.clone()).or_insert(vec![]);
                    timevec.insert(0, ts-start_ts);
                }
            }
            _ => assert!(false, "unexpected {:?}", p),
        }
    }

    // Parse list of pending merges (from stdin or from emerge log filtered by cms).
    // We collect immediately to deal with type mismatches; it should be a small list anyway.
    let pretend: Vec<Parsed> = match atty::is(atty::Stream::Stdin) {
        false => Parser::new_pretend(stdin(), "STDIN")?.collect(),
        true => started.iter()
            .filter(|&(_,t)| *t > cms)
            .map(|(&(ref e,ref v),_)| Parsed::Pretend{ebuild:e.to_string(), version:v.to_string(), line: String::from("")})
            .collect(),
    };

    // Gather and print per-package and indivudual stats.
    let mut totcount = 0;
    let mut totunknown = 0;
    let mut totpredict = 0;
    let mut totelapsed = 0;
    for p in pretend {
        match p {
            Parsed::Pretend{ebuild, version, ..} => {
                totcount += 1;
                if let Some(tv) = times.get(&ebuild) {
                    let (predtime,predcount,_) = tv.iter()
                        .fold((0,0,0), |(pt,pc,tc), &i| {
                            if tc >= lim {(pt,  pc,  tc+1)}
                            else         {(pt+i,pc+1,tc+1)}
                        });
                    totpredict += predtime / predcount;
                    match started.remove(&(ebuild.clone(), version.clone())) {
                        Some(start_ts) if start_ts > cms => {
                            // There's an emerge process running since before this unfinished merge started,
                            // so a good guess is that this merge is currently running. This can lead to
                            // false-positives, but is IMHO no worse than genlop's heuristics.
                            totelapsed += now - start_ts;
                            totpredict -= std::cmp::min(predtime / predcount, now - start_ts);
                            writeln!(tw, "{}\t{:>9} - {}", ebuild, fmt_duration(predtime/predcount), fmt_duration(now-start_ts))?;
                        },
                        _ =>
                            writeln!(tw, "{}\t{:>9}", ebuild, fmt_duration(predtime/predcount))?,
                    }
                } else {
                    totunknown += 1;
                    writeln!(tw, "{}\t", ebuild)?;
                }
            },
            _ => assert!(false, "unexpected {:?}", p),
        }
    }
    if totcount > 0 {
        writeln!(tw, "Estimate for {} ebuilds ({} unknown, {} elapsed)\t{:>9}", totcount, totunknown, fmt_duration(totelapsed), fmt_duration(totpredict))?;
    } else {
        writeln!(tw, "No pretended merge found")?;
    }
    Ok(totcount > 0)
}


#[cfg(test)]
mod tests {
    use assert_cli::Assert;

    #[test]
    fn predict_tty() {
        // This depends on there being no currently running emerge.
        // Not a hugely useful test, but it's something.
        let o = "No pretended merge found\n";
        Assert::main_binary().with_args(&["p"]).fails_with(2).stdout().is(o).unwrap();
    }

    #[test]
    fn predict_emerge_p() {
        let t = vec![
            // Check garbage input
            (indoc!("blah blah\n"),
             indoc!("No pretended merge found\n"),
             2),
            // Check all-unknowns
            (indoc!("[ebuild   R   ~] dev-lang/unknown-1.42\n"),
             indoc!("dev-lang/unknown                               \n\
                     Estimate for 1 ebuilds (1 unknown, 0 elapsed)          0\n"),
             0),
            // Check that unknown ebuild don't wreck allignment. Remember that times are {:>9}
            (indoc!("[ebuild   R   ~] dev-qt/qtcore-5.9.4-r2\n\
                     [ebuild   R   ~] dev-lang/unknown-1.42\n\
                     [ebuild   R   ~] dev-qt/qtgui-5.9.4-r3\n"),
             indoc!("dev-qt/qtcore                                       3:44\n\
                     dev-lang/unknown                               \n\
                     dev-qt/qtgui                                        4:36\n\
                     Estimate for 3 ebuilds (1 unknown, 0 elapsed)       8:20\n"),
             0),
        ];
        for (i,o,e) in t {
            match e { //FIXME: Simplify code once https://github.com/assert-rs/assert_cli/issues/99 is closed
                0 => Assert::main_binary().with_args(&["-f","test/emerge.10000.log","p"])
                    .stdin(i).stdout().is(o).unwrap(),
                _ => Assert::main_binary().with_args(&["-f","test/emerge.10000.log","p"])
                    .fails_with(e).stdin(i).stdout().is(o).unwrap(),
            }
        }
    }

    #[test]
    fn stats() {
        let t = vec![
            (&["-f","test/emerge.10000.log","s","client"],
             indoc!("kde-frameworks/kxmlrpcclient         47    2        23\n\
                     mail-client/thunderbird         1:23:44    2     41:52\n\
                     www-client/chromium            21:41:24    3   7:13:48\n\
                     www-client/falkon                  6:02    1      6:02\n\
                     www-client/firefox                47:29    1     47:29\n\
                     www-client/links                     44    1        44\n\
                     x11-apps/xlsclients                  14    1        14\n"),
             0),
        ];
        for (a,o,e) in t {
            match e { //FIXME: Simplify code once https://github.com/assert-rs/assert_cli/issues/99 is closed
                0 => Assert::main_binary().with_args(a).stdout().is(o).unwrap(),
                _ => Assert::main_binary().with_args(a).fails_with(e).stdout().is(o).unwrap(),
            }
        }
    }

    #[test]
    fn exit_status() {
        // 0: no problem
        // 1: user or program error
        // 2: command ran properly but didn't find anything
        let t: Vec<(&[&str],i32)> = vec![
            // Help, version, badarg (clap)
            (&["-h"], 0),
            (&["-V"], 0),
            (&["l","-h"], 0),
            (&[], 1),
            (&["s","--foo"], 1),
            (&["badcmd"], 1),
            // Bad arguments (emlop)
            (&["l","--logfile","notfound"], 1),
            (&["s","--logfile","notfound"], 1),
            (&["p","--logfile","notfound"], 1),
            (&["l","bad regex [a-z"], 1),
            (&["s","bad regex [a-z"], 1),
            (&["p","bad regex [a-z"], 1),
            // Normal behaviour
            (&["-f","test/emerge.10000.log","p"], 2),
            (&["-f","test/emerge.10000.log","l"], 0),
            (&["-f","test/emerge.10000.log","l","-e","icu"], 0),
            (&["-f","test/emerge.10000.log","l","-e","unknown"], 2),
            (&["-f","test/emerge.10000.log","s"], 0),
            (&["-f","test/emerge.10000.log","s","-e","icu"], 0),
            (&["-f","test/emerge.10000.log","s","-e","unknown"], 2),
        ];
        for (args, exit) in t {
            match exit {
                0 => Assert::main_binary().with_args(args).unwrap(),
                _ => Assert::main_binary().with_args(args).fails_with(exit).unwrap(),
            }
        }
    }
}
