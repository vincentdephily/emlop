complete -c emlop -e
complete -c emlop -f
complete -c emlop -s f -l from -d 'Only parse log entries after <date>' -x -a "{1y	'One year ago',1m	'One month ago',1w	'One week ago',1d	'One day ago',1h	'One hour ago',(date -Is)	'Exact date'}"
complete -c emlop -s t -l to -d 'Only parse log entries before <date>' -x -a "{1y	'One year ago',1m	'One month ago',1w	'One week ago',1d	'One day ago',1h	'One hour ago',(date -Is)	'Exact date'}"
complete -c emlop -s H -l header -d 'Show table header' -f -a "yes no"
complete -c emlop -l elipsis -d 'Show skipped rows' -f -a "yes no"
complete -c emlop -l duration -d 'Output durations in different formats' -x -a "hms hmsfixed human secs"
complete -c emlop -l date -d 'Output dates in different formats' -x -a "ymd ymdhms ymdhmso rfc3339 rfc2822 compact unix"
complete -c emlop -l utc -d 'Parse/display dates in UTC instead of local time' -f -a "yes no"
complete -c emlop -l color -d 'Enable color (yes/no/auto)' -f -a "{yes	Enabled,no	Disabled,auto	'Enabled on terminal'}"
complete -c emlop -s o -l output -d 'Ouput format' -x -a "columns tab auto"
complete -c emlop -s F -l logfile -d 'Location of emerge log file' -r -F
complete -c emlop -s v -x -a "{	'Show warnings',v	'Show info',vv	'Show debug',vvv	'Show trace'}" -d 'Increase verbosity'
complete -c emlop -s h -d 'Print short help'
complete -c emlop -l help -d 'Print long help'
complete -c emlop -n "__fish_use_subcommand" -s V -l version -d 'Print version'

complete -c emlop -n "__fish_use_subcommand" -f -a "log" -d 'Show log of sucessful merges, unmerges and syncs'
complete -c emlop -n "__fish_use_subcommand" -f -a "predict" -d 'Predict merge times for current or pretended merges'
complete -c emlop -n "__fish_use_subcommand" -f -a "stats" -d 'Show statistics about syncs, per-package (un)merges, and total (un)merges'
complete -c emlop -n "__fish_use_subcommand" -f -a "accuracy" -d 'Compare actual merge time against predicted merge time'

complete -c emlop -n "__fish_seen_subcommand_from log" -l starttime -d 'Display start time instead of end time' -f -a "yes no"
complete -c emlop -n "__fish_seen_subcommand_from log" -s N -l first -d 'Show only the first <num> entries' -f -a "{	'Show only first entry',5	'Show only first 5 entries',10	'Show only first 10 entries'}"
complete -c emlop -n "__fish_seen_subcommand_from log" -s n -l last -d 'Show only the last <num> entries' -f -a "{	'Show only last entry',5	'Show only last 5 entries',10	'Show only last 10 entries'}"
complete -c emlop -n "__fish_seen_subcommand_from log" -s s -l show -d 'Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll' -x -a "musa"
complete -c emlop -n "__fish_seen_subcommand_from log" -s e -l exact -d 'Match <search> using plain string'
complete -c emlop -n "__fish_seen_subcommand_from log" -a "(emlop complete '$1')"

complete -c emlop -n "__fish_seen_subcommand_from predict" -s s -l show -d 'Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll' -x -a "emta"
complete -c emlop -n "__fish_seen_subcommand_from predict" -s N -l first -d 'Show only the first <num> entries' -f -a "{	'Show only first entry',5	'Show only first 5 entries',10	'Show only first 10 entries'}"
complete -c emlop -n "__fish_seen_subcommand_from predict" -s n -l last -d 'Show only the last <num> entries' -f -a "{	'Show only last entry',5	'Show only last 5 entries',10	'Show only last 10 entries'}"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l tmpdir -d 'Location of portage tmpdir' -x -a "(__fish_complete_directories '$1')"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l resume -d 'Use main, backup, either, or no portage resume list' -f -a "{auto	'',either	'',main	'',backup	'',no	''}"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l unknown -d 'Assume unkown packages take <secs> seconds to merge' -x -a "0 5 10 20 60"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l avg -d 'Select function used to predict durations' -x -a "arith median weighted-arith weighted-median"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l limit -d 'Use the last <num> merge times to predict durations' -x -a "1 5 20 999"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l pwidth -d 'Maximum width of emerge proces comandline' -x -a "10 20 40 80 160"
complete -c emlop -n "__fish_seen_subcommand_from predict" -l pdepth -d 'Maximum depth of emerge proces tree' -x -a "0 1 3 5 7 99"

complete -c emlop -n "__fish_seen_subcommand_from stats" -s s -l show -d 'Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll' -x -a "ptsa"
complete -c emlop -n "__fish_seen_subcommand_from stats" -s g -l groupby -d 'Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one' -x -a "year month week day none"
complete -c emlop -n "__fish_seen_subcommand_from stats" -l avg -d 'Select function used to predict durations' -x -a "arith median weighted-arith weighted-median"
complete -c emlop -n "__fish_seen_subcommand_from stats" -l limit -d 'Use the last <num> merge times to predict durations' -x -a "1 5 20 999"
complete -c emlop -n "__fish_seen_subcommand_from stats" -s e -l exact -d 'Match <search> using plain string'
complete -c emlop -n "__fish_seen_subcommand_from stats" -a "(emlop complete '$1')"

complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s s -l show -d 'Show (m)erges, (t)otals, and/or (a)ll' -x -a "mta"
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s n -l last -d 'Show only the last <num> entries' -x -a "{	'Show only last entry',5	'Show only last 5 entries',10	'Show only last 10 entries'}"
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l avg -d 'Select function used to predict durations' -x -a "arith median weighted-arith weighted-median"
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -l limit -d 'Use the last <num> merge times to predict durations' -x -a "1 5 20 999"
complete -c emlop -n "__fish_seen_subcommand_from accuracy" -s e -l exact -d 'Match <search> using plain string'
