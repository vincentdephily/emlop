use clap::ArgMatches;
use std::collections::HashMap;
use std::io;

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
pub fn cmd_summary(args: &ArgMatches, subargs: &ArgMatches) -> Result<(), io::Error> {
    let parser = HistParser::new(args.value_of("logfile").unwrap(), subargs.value_of("package"));
    let lim = value_t!(subargs, "limit", usize).unwrap();
    let mut started: HashMap<(String, String, String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    let mut maxlen = 0;
    for event in parser {
        match event {
            HistEvent::Start{ts, ebuild, version, iter, ..} => {
                started.insert((ebuild.clone(), version.clone(), iter.clone()), ts);
            },
            HistEvent::Stop{ts, ebuild, version, iter, ..} => {
                match started.remove(&(ebuild.clone(), version.clone(), iter.clone())) {
                    Some(start_ts) => {
                        maxlen = maxlen.max(ebuild.len());
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
        println!("{:width$} {:>9}/{:<4} {:>8}", pkg, fmt_duration(tottime), totcount, fmt_duration(predtime/predcount), width=maxlen);
    }
    Ok(())
}

/// Predict future merge time
///
/// Very similar to cmd_summary except we want total build time for a list of ebuilds.
pub fn cmd_predict(args: &ArgMatches, subargs: &ArgMatches) -> Result<(), io::Error> {
    let now = epoch_now();
    let lim = value_t!(subargs, "limit", usize).unwrap();

    // Gather and print info about current merge process.
    let cms = current_merge_start();

    // Parse emerge log.
    let hist = HistParser::new(args.value_of("logfile").unwrap(), None);
    let mut started: HashMap<(String, String), i64> = HashMap::new();
    let mut times: HashMap<String, Vec<i64>> = HashMap::new();
    let mut maxlen = 0;
    for event in hist {
        match event {
            // We're ignoring iter here (reducing the start->stop matching accuracy) because there's no iter in the pretend output.
            HistEvent::Start{ts, ebuild, version, ..} => {
                started.insert((ebuild.clone(), version.clone()), ts);
            }
            HistEvent::Stop{ts, ebuild, version, ..} => {
                if let Some(start_ts) = started.remove(&(ebuild.clone(), version.clone())) {
                    maxlen = maxlen.max(ebuild.len());//FIXME compute maxlen for displayed packages only
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
                    println!("{:width$} {:>9} - {}", ebuild, fmt_duration(predtime/predcount), fmt_duration(now-start_ts), width=maxlen);
                },
                _ =>
                    println!("{:width$} {:>9}", ebuild, fmt_duration(predtime/predcount), width=maxlen),
            }
        } else {
            totunknown += 1;
            totcount += 1;
            println!("{:width$}", ebuild, width=maxlen);
        }
    }
    if totcount > 0 {
        println!("Estimate for {} ebuilds ({} unknown, {} elapsed)   {}", totcount, totunknown, fmt_duration(totelapsed), fmt_duration(tottime-totelapsed));
    } else {
        println!("No ongoing or pretended merges found");
    }
    Ok(())
}
