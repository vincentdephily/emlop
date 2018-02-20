use std::collections::HashMap;

use ::*;
use parser::*;
use proces::*;

/// Straightforward display of merge events
///
/// We store the start times in a hashmap to compute/print the duration when we reach a stop event.
pub fn cmd_list(args: &ArgMatches, subargs: &ArgMatches) -> Result<(), io::Error> {
    let hist = HistParser::new(args.value_of("logfile").unwrap(), subargs.value_of("package"));
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    for event in hist {
        match event {
            HistEvent::Start{ts, ebuild, version, iter, ..} => {
                // This'll overwrite any previous entry, if a build started but never finished
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            HistEvent::Stop{ts, ebuild, version, iter, ..} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(prevts) => println!("{} {:>9} {}-{}",     fmt_time(ts), fmt_duration(ts - prevts), ebuild, version),
                    None =>         println!("{}  00:00:00 {}-{}", fmt_time(ts), ebuild, version),
                }
            },
        }
    };
    Ok(())
}

/// Summary display of merge events
///
/// First loop is like cmd_list but we store the merge time for each ebuild instead of printing it.
/// Then we compute the stats per ebuild, and print that.
pub fn cmd_summary(tw: &mut TabWriter<io::Stdout>, args: &ArgMatches, subargs: &ArgMatches) -> Result<(), io::Error> {
    let parser = HistParser::new(args.value_of("logfile").unwrap(), subargs.value_of("package"));
    let lim = value_t!(subargs, "limit", usize).unwrap();
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    for event in parser {
        match event {
            HistEvent::Start{ts, ebuild, version, iter, ..} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            HistEvent::Stop{ts, ebuild, version, iter, ..} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(start_ts) => {
                        let timevec = times.entry(ebuild.clone()).or_insert(vec![]);
                        timevec.insert(0, ts-start_ts);
                    },
                    None => (),
                }
            }
        }
    };
    for (pkg,tv) in times.iter() {
        let (predtime,predcount,tottime,totcount) = tv.iter()
            .fold((0,0,0,0), |(pt,pc,tt,tc), &i| {
                if tc >= lim {(pt,  pc,  tt+i,tc+1)}
                else         {(pt+i,pc+1,tt+i,tc+1)}
            });
        writeln!(tw, "{}\t{:>9}\t{:>3}\t{:>8}", pkg, fmt_duration(tottime), totcount, fmt_duration(predtime/predcount))?;
    }
    Ok(())
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(tw: &mut TabWriter<io::Stdout>, args: &ArgMatches, subargs: &ArgMatches) -> Result<(), io::Error> {
    let now = epoch_now();
    let lim = value_t!(subargs, "limit", usize).unwrap();

    // Gather and print info about current merge process.
    let mut cms = std::i64::MAX;//current_merge_start(tw)?;
    for i in get_all_info(Some("emerge"))? {
        cms = std::cmp::min(cms, i.start);
        writeln!(tw, "Pid {}: ...{}\t{:>9}", i.pid, &i.cmdline[(i.cmdline.len()-35)..], fmt_duration(now-i.start))?;
    }

    // Parse emerge log.
    let hist = HistParser::new(args.value_of("logfile").unwrap(), None);
    let mut started: HashMap<(String, String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    for event in hist {
        match event {
            // We're ignoring iter here (reducing the start->stop matching accuracy) because there's no iter in the pretend output.
            HistEvent::Start{ts, ebuild, version, ..} => {
                started.insert((ebuild.clone(), version.clone()), ts);
            }
            HistEvent::Stop{ts, ebuild, version, ..} => {
                if let Some(start_ts) = started.remove(&(ebuild.clone(), version.clone())) {
                    let timevec = times.entry(ebuild.clone()).or_insert(vec![]);
                    timevec.insert(0, ts-start_ts);
                }
            }
        }
    }

    // Parse list of pending merges (from stdin or from emerge log filterd by cms).
    // We collect immediately to deal with type mismatches; it should be a small list anyway.
    let pretend: Vec<PretendEvent> = match atty::is(atty::Stream::Stdin) {
        false => PretendParser::new().collect(),
        true => started.iter()
            .filter(|&(_,t)| *t > cms)
            .map(|(&(ref  e,ref v),_)| PretendEvent{ebuild:e.to_string(), version:v.to_string()})
            .collect(),
    };

    // Gather and print per-package and indivudual stats.
    let mut tottime = 0;
    let mut totcount = 0;
    let mut totunknown = 0;
    let mut totelapsed = 0;
    for PretendEvent{ebuild, version} in pretend {
        if let Some(tv) = times.get(&ebuild) {
            let (predtime,predcount,_) = tv.iter()
                .fold((0,0,0), |(pt,pc,tc), &i| {
                    if tc >= lim {(pt,  pc,  tc+1)}
                    else         {(pt+i,pc+1,tc+1)}
                });
            tottime += predtime / predcount;
            totcount += 1;
            match started.remove(&(ebuild.clone(), version.clone())) {
                Some(start_ts) if start_ts > cms => {
                    // There's an emerge process running since before this unfinished merge started,
                    // so a good guess is that this merge is currently running. This can lead to
                    // false-positives, but is IMHO no worse than genlop's heuristics.
                    totelapsed += now - start_ts;
                    writeln!(tw, "{}\t{:>9} - {}", ebuild, fmt_duration(predtime/predcount), fmt_duration(now-start_ts))?;
                },
                _ =>
                    writeln!(tw, "{}\t{:>9}", ebuild, fmt_duration(predtime/predcount))?,
            }
        } else {
            totunknown += 1;
            totcount += 1;
            writeln!(tw, "{}", ebuild)?;
        }
    }
    if totcount > 0 {
        writeln!(tw, "Estimate for {} ebuilds ({} unknown, {} elapsed)\t{:>9}", totcount, totunknown, fmt_duration(totelapsed), fmt_duration(tottime-totelapsed))?;
    } else {
        writeln!(tw, "No ongoing or pretended merges found")?;
    }
    Ok(())
}
