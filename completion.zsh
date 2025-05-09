#compdef emlop

_emlop() {
    typeset -A opt_args
    local context curcontext="$curcontext" state line

    _arguments -s -S -C \
'-f+[Only parse log entries after <date>]:date: ' \
'--from=[Only parse log entries after <date>]:date: ' \
'-t+[Only parse log entries before <date>]:date: ' \
'--to=[Only parse log entries before <date>]:date: ' \
'-H+[Show table header]' \
'--header=[Show table header]' \
'--showskip=[Show skipped rows]' \
'--duration=[Output durations in different formats]:format: ' \
'--date=[Output dates in different formats]:format: ' \
'--utc=[Parse/display dates in UTC instead of local time]' \
'--color=[Enable color (yes/no/auto)]' \
'--theme=[Set terminal colors]' \
'-o+[Output format (columns/tab/auto)]:format: ' \
'--output=[Output format (columns/tab/auto)]:format: ' \
'-F+[Location of emerge log file]:file: ' \
'--logfile=[Location of emerge log file]:file: ' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_emlop_commands" \
"*::: :->emlop"
    case $state in
    (emlop)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:emlop-command-$line[1]:"
        case $line[1] in
            (log)
                _arguments -s -S -C \
'--starttime=[Display start time instead of end time]' \
'-N+[Show only the first <num> entries]' \
'--first=[Show only the first <num> entries]' \
'-n+[Show only the last <num> entries]' \
'--last=[Show only the last <num> entries]' \
'-s+[Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll]:m,u,s,a: ' \
'--show=[Show (m)erges, (u)nmerges, (s)yncs, and/or (a)ll]:m,u,s,a: ' \
'-f+[Only parse log entries after <date>]:date: ' \
'--from=[Only parse log entries after <date>]:date: ' \
'-t+[Only parse log entries before <date>]:date: ' \
'--to=[Only parse log entries before <date>]:date: ' \
'-H+[Show table header]' \
'--header=[Show table header]' \
'--showskip=[Show skipped rows]' \
'--duration=[Output durations in different formats]:format: ' \
'--date=[Output dates in different formats]:format: ' \
'--utc=[Parse/display dates in UTC instead of local time]' \
'--color=[Enable color (yes/no/auto)]' \
'--theme=[Set terminal colors]' \
'-o+[Output format (columns/tab/auto)]:format: ' \
'--output=[Output format (columns/tab/auto)]:format: ' \
'-F+[Location of emerge log file]:file: ' \
'--logfile=[Location of emerge log file]:file: ' \
'-e[Match <search> using plain string]' \
'--exact[Match <search> using plain string]' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::search:($(emlop complete))'
                ;;
            (predict)
                _arguments -s -S -C \
'-s+[Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll]:e,m,t,a: ' \
'--show=[Show (e)emerge processes, (m)erges, (t)otal, and/or (a)ll]:e,m,t,a: ' \
'-N+[Show only the first <num> entries]' \
'--first=[Show only the first <num> entries]' \
'-n+[Show only the last <num> entries]' \
'--last=[Show only the last <num> entries]' \
'*--tmpdir=[Location of portage tmpdir]:dir:_files' \
'--resume=[Use main, backup, either, or no portage resume list]' \
'--unknownb=[Assume unkown binary packages take <secs> seconds to merge]:secs: ' \
'--unknownc=[Assume unkown compiled packages take <secs> seconds to merge]:secs: ' \
'--avg=[Select function used to predict durations]:fn: ' \
'--limit=[Use the last <num> merge times to predict durations]:num: ' \
'-f+[Only parse log entries after <date>]:date: ' \
'--from=[Only parse log entries after <date>]:date: ' \
'-t+[Only parse log entries before <date>]:date: ' \
'--to=[Only parse log entries before <date>]:date: ' \
'-H+[Show table header]' \
'--header=[Show table header]' \
'--showskip=[Show skipped rows]' \
'--duration=[Output durations in different formats]:format: ' \
'--date=[Output dates in different formats]:format: ' \
'--utc=[Parse/display dates in UTC instead of local time]' \
'--color=[Enable color (yes/no/auto)]' \
'--theme=[Set terminal colors]' \
'-o+[Output format (columns/tab/auto)]:format: ' \
'--output=[Output format (columns/tab/auto)]:format: ' \
'-W+[Maximum width of emerge proces comandline]'
'--pwidth=[Maximum width of emerge proces comandline]'
'-D+[Maximum depth of emerge proces tree]'
'--pdepth=[Maximum depth of emerge proces tree]'
'-F+[Location of emerge log file]:file: ' \
'--logfile=[Location of emerge log file]:file: ' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]'
                ;;
            (stats)
                _arguments -s -S -C \
'-s+[Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll]:p,t,s,a: ' \
'--show=[Show (p)ackages, (t)otals, (s)yncs, and/or (a)ll]:p,t,s,a: ' \
'-g+[Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one]:y,m,w,d,n: ' \
'--groupby=[Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one]:y,m,w,d,n: ' \
'--avg=[Select function used to predict durations]:fn: ' \
'--limit=[Use the last <num> merge times to predict durations]:num: ' \
'-f+[Only parse log entries after <date>]:date: ' \
'--from=[Only parse log entries after <date>]:date: ' \
'-t+[Only parse log entries before <date>]:date: ' \
'--to=[Only parse log entries before <date>]:date: ' \
'-H+[Show table header]' \
'--header=[Show table header]' \
'--showskip=[Show skipped rows]' \
'--duration=[Output durations in different formats]:format: ' \
'--date=[Output dates in different formats]:format: ' \
'--utc=[Parse/display dates in UTC instead of local time]' \
'--color=[Enable color (yes/no/auto)]' \
'--theme=[Set terminal colors]' \
'-o+[Output format (columns/tab/auto)]:format: ' \
'--output=[Output format (columns/tab/auto)]:format: ' \
'-F+[Location of emerge log file]:file: ' \
'--logfile=[Location of emerge log file]:file: ' \
'-e[Match <search> using plain string]' \
'--exact[Match <search> using plain string]' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::search:($(emlop complete))'
                ;;
            (accuracy)
                _arguments -s -S -C \
'-s+[Show (m)erges, (t)otals, and/or (a)ll]:m,t,a: ' \
'--show=[Show (m)erges, (t)otals, and/or (a)ll]:m,t,a: ' \
'-n+[Show only the last <num> entries]' \
'--last=[Show only the last <num> entries]' \
'--avg=[Select function used to predict durations]:fn: ' \
'--limit=[Use the last <num> merge times to predict durations]:num: ' \
'-f+[Only parse log entries after <date>]:date: ' \
'--from=[Only parse log entries after <date>]:date: ' \
'-t+[Only parse log entries before <date>]:date: ' \
'--to=[Only parse log entries before <date>]:date: ' \
'-H+[Show table header]' \
'--header=[Show table header]' \
'--showskip=[Show skipped rows]' \
'--duration=[Output durations in different formats]:format: ' \
'--date=[Output dates in different formats]:format: ' \
'--utc=[Parse/display dates in UTC instead of local time]' \
'--color=[Enable color (yes/no/auto)]' \
'--theme=[Set terminal colors]' \
'-o+[Output format (columns/tab/auto)]:format: ' \
'--output=[Output format (columns/tab/auto)]:format: ' \
'-F+[Location of emerge log file]:file: ' \
'--logfile=[Location of emerge log file]:file: ' \
'-e[Match <search> using plain string]' \
'--exact[Match <search> using plain string]' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::search:($(emlop complete))'
                ;;
        esac
        ;;
    esac
}

(( $+functions[_emlop_commands] )) ||
_emlop_commands() {
    local commands; commands=(
'log:Show log of sucessful merges, unmerges and syncs' \
'predict:Predict merge times for current or pretended merges' \
'stats:Show statistics about syncs, per-package (un)merges, and total (un)merges' \
'accuracy:Compare actual merge time against predicted merge time' \
    )
    _describe -t commands 'emlop commands' commands "$@"
}
(( $+functions[_emlop__accuracy_commands] )) ||
_emlop__accuracy_commands() {
    local commands; commands=()
    _describe -t commands 'emlop accuracy commands' commands "$@"
}
(( $+functions[_emlop__log_commands] )) ||
_emlop__log_commands() {
    local commands; commands=()
    _describe -t commands 'emlop log commands' commands "$@"
}
(( $+functions[_emlop__predict_commands] )) ||
_emlop__predict_commands() {
    local commands; commands=()
    _describe -t commands 'emlop predict commands' commands "$@"
}
(( $+functions[_emlop__stats_commands] )) ||
_emlop__stats_commands() {
    local commands; commands=()
    _describe -t commands 'emlop stats commands' commands "$@"
}

if [ "$funcstack[1]" = "_emlop" ]; then
    _emlop "$@"
else
    compdef _emlop emlop
fi
