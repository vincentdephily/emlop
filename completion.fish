complete -c emlop -n "__fish_use_subcommand" -s f -l from -d 'Only parse log entries after <date>' -r
complete -c emlop -n "__fish_use_subcommand" -s t -l to -d 'Only parse log entries before <date>' -r
complete -c emlop -n "__fish_use_subcommand" -s H -l header -d 'Show table header' -r
complete -c emlop -n "__fish_use_subcommand" -l duration -d 'Output durations in different formats' -r
complete -c emlop -n "__fish_use_subcommand" -l date -d 'Output dates in different formats' -r
complete -c emlop -n "__fish_use_subcommand" -l utc -d 'Parse/display dates in UTC instead of local time' -r
complete -c emlop -n "__fish_use_subcommand" -l color -d 'Enable color (yes/no/auto)' -r
complete -c emlop -n "__fish_use_subcommand" -s o -l output -d 'Ouput format (columns/tab/auto)' -r
complete -c emlop -n "__fish_use_subcommand" -s F -l logfile -d 'Location of emerge log file' -r
complete -c emlop -n "__fish_use_subcommand" -s v -d 'Increase verbosity (can be given multiple times)'
complete -c emlop -n "__fish_use_subcommand" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c emlop -n "__fish_use_subcommand" -s V -l version -d 'Print version'
complete -c emlop -n "__fish_use_subcommand" -f -a "log" -d 'Show log of sucessful merges, unmerges and syncs'
complete -c emlop -n "__fish_use_subcommand" -f -a "predict" -d 'Predict merge times for current or pretended merges'
complete -c emlop -n "__fish_use_subcommand" -f -a "stats" -d 'Show statistics about syncs, per-package (un)merges, and total (un)merges'
complete -c emlop -n "__fish_use_subcommand" -f -a "accuracy" -d 'Compare actual merge time against predicted merge time'
complete -c emlop -n "__fish_seen_subcommand_from log" -l starttime -d 'Display start time instead of end time' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s N -l first -d 'Show only the first <num> entries' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s n -l last -d 'Show only the last <num> entries' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s s -l show -d 'Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s f -l from -d 'Only parse log entries after <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s t -l to -d 'Only parse log entries before <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s H -l header -d 'Show table header' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -l duration -d 'Output durations in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -l date -d 'Output dates in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -l utc -d 'Parse/display dates in UTC instead of local time' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -l color -d 'Enable color (yes/no/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s o -l output -d 'Ouput format (columns/tab/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s F -l logfile -d 'Location of emerge log file' -r
complete -c emlop -n "__fish_seen_subcommand_from log" -s e -l exact -d 'Match <search> using plain string'
complete -c emlop -n "__fish_seen_subcommand_from log" -s v -d 'Increase verbosity (can be given multiple times)'
complete -c emlop -n "__fish_seen_subcommand_from log" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c emlop -n "__fish_seen_subcommand_from predict" -s s -l show -d 'Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s N -l first -d 'Show only the first <num> entries' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s n -l last -d 'Show only the last <num> entries' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l tmpdir -d 'Location of portage tmpdir' -r -F
complete -c emlop -n "__fish_seen_subcommand_from predict" -l resume -d 'Use main, backup, either, or no portage resume list' -r -f -a "{auto	'',either	'',main	'',backup	'',no	''}"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l unknown -d 'Assume unkown packages take <secs> seconds to merge' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l avg -d 'Select function used to predict durations' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l limit -d 'Use the last <num> merge times to predict durations' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s f -l from -d 'Only parse log entries after <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s t -l to -d 'Only parse log entries before <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s H -l header -d 'Show table header' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l duration -d 'Output durations in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l date -d 'Output dates in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l utc -d 'Parse/display dates in UTC instead of local time' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -l color -d 'Enable color (yes/no/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s o -l output -d 'Ouput format (columns/tab/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s F -l logfile -d 'Location of emerge log file' -r
complete -c emlop -n "__fish_seen_subcommand_from predict" -s v -d 'Increase verbosity (can be given multiple times)'
complete -c emlop -n "__fish_seen_subcommand_from predict" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c emlop -n "__fish_seen_subcommand_from stats" -s s -l show -d 'Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s g -l groupby -d 'Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -l avg -d 'Select function used to predict durations' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -l limit -d 'Use the last <num> merge times to predict durations' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s f -l from -d 'Only parse log entries after <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s t -l to -d 'Only parse log entries before <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s H -l header -d 'Show table header' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -l duration -d 'Output durations in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -l date -d 'Output dates in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -l utc -d 'Parse/display dates in UTC instead of local time' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -l color -d 'Enable color (yes/no/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s o -l output -d 'Ouput format (columns/tab/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s F -l logfile -d 'Location of emerge log file' -r
complete -c emlop -n "__fish_seen_subcommand_from stats" -s e -l exact -d 'Match <search> using plain string'
complete -c emlop -n "__fish_seen_subcommand_from stats" -s v -d 'Increase verbosity (can be given multiple times)'
complete -c emlop -n "__fish_seen_subcommand_from stats" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s s -l show -d 'Show (m)erges, (t)otals, and/or (a)ll' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s n -l last -d 'Show only the last <num> entries' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l avg -d 'Select function used to predict durations' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l limit -d 'Use the last <num> merge times to predict durations' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s f -l from -d 'Only parse log entries after <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s t -l to -d 'Only parse log entries before <date>' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s H -l header -d 'Show table header' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l duration -d 'Output durations in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l date -d 'Output dates in different formats' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l utc -d 'Parse/display dates in UTC instead of local time' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l color -d 'Enable color (yes/no/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s o -l output -d 'Ouput format (columns/tab/auto)' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s F -l logfile -d 'Location of emerge log file' -r
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s e -l exact -d 'Match <search> using plain string'
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s v -d 'Increase verbosity (can be given multiple times)'
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s h -l help -d 'Print help (see more with \'--help\')'
