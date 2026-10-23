%#compdef emlop

_emlop() {
    typeset -A opt_args
    local context curcontext="$curcontext" state line

    _arguments -s -S -C \
'-f+[Only parse log entries after <date/command>]:date:_default' \
'--from=[Only parse log entries after <date/command>]:date:_default' \
'-t+[Only parse log entries before <date/command>]:date:_default' \
'--to=[Only parse log entries before <date/command>]:date:_default' \
'-H+[Show table header]::bool:_default' \
'--header=[Show table header]::bool:_default' \
'--duration=[Output durations in different formats]:format:_default' \
'--date=[Output dates in different formats]:format:_default' \
'--utc=[Parse/display dates in UTC instead of local time]::bool:_default' \
'--color=[Enable color (yes/no/auto)]::bool:_default' \
'-o+[Output format (columns/tab/auto)]:format:_default' \
'--output=[Output format (columns/tab/auto)]:format:_default' \
'-F+[Location of emerge log file]:file:_default' \
'--logfile=[Location of emerge log file]:file:_default' \
'-S+[Show number of skipped rows (yes/no)]::bool:_default' \
'--showskip=[Show number of skipped rows (yes/no)]::bool:_default' \
'--theme=[Set terminal colors]:key_SGR:_default' \
'--tty=[Assume stdin/stdout is a terminal (in/out/inout/none/auto)]::inout:(auto in out inout none)' \
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
'--starttime=[Display start time instead of end time]::bool:_default' \
'-N+[Show only the first <num> entries]::num:_default' \
'--first=[Show only the first <num> entries]::num:_default' \
'-n+[Show only the last <num> entries]::num:_default' \
'--last=[Show only the last <num> entries]::num:_default' \
'-s+[Show emerge (r)uns, (m)erges, (u)nmerges, (s)yncs, and/or (a)ll]:r,m,u,s,a:_default' \
'--show=[Show emerge (r)uns, (m)erges, (u)nmerges, (s)yncs, and/or (a)ll]:r,m,u,s,a:_default' \
'-f+[Only parse log entries after <date/command>]:date:_default' \
'--from=[Only parse log entries after <date/command>]:date:_default' \
'-t+[Only parse log entries before <date/command>]:date:_default' \
'--to=[Only parse log entries before <date/command>]:date:_default' \
'-H+[Show table header]::bool:_default' \
'--header=[Show table header]::bool:_default' \
'--duration=[Output durations in different formats]:format:_default' \
'--date=[Output dates in different formats]:format:_default' \
'--utc=[Parse/display dates in UTC instead of local time]::bool:_default' \
'--color=[Enable color (yes/no/auto)]::bool:_default' \
'-o+[Output format (columns/tab/auto)]:format:_default' \
'--output=[Output format (columns/tab/auto)]:format:_default' \
'-F+[Location of emerge log file]:file:_default' \
'--logfile=[Location of emerge log file]:file:_default' \
'-S+[Show number of skipped rows (yes/no)]::bool:_default' \
'--showskip=[Show number of skipped rows (yes/no)]::bool:_default' \
'--theme=[Set terminal colors]:key_SGR:_default' \
'--tty=[Assume stdin/stdout is a terminal (in/out/inout/none/auto)]::inout:(auto in out inout none)' \
'-e[Match <search> using plain string]' \
'--exact[Match <search> using plain string]' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::search:($(emlop complete))'
                ;;
            (predict)
                _arguments -s -S -C \
'-s+[Show (r)unning processes, (m)erges, (t)otal, and/or (a)ll]:r,m,t,a:_default' \
'--show=[Show (r)unning processes, (m)erges, (t)otal, and/or (a)ll]:r,m,t,a:_default' \
'-N+[Show only the first <num> entries]::num:_default' \
'--first=[Show only the first <num> entries]::num:_default' \
'-n+[Show only the last <num> entries]::num:_default' \
'--last=[Show only the last <num> entries]::num:_default' \
'*--tmpdir=[Location of portage tmpdir]:dir:_files' \
'--mtimedbfile=[Location of portage mtimedb file]:file:_default' \
'--resume=[Use main, backup, either, or no portage resume list]::source:(auto either main backup no)' \
'--unknownc=[Assume unkown compiled packages take <secs> seconds to merge]:secs:_default' \
'--unknownb=[Assume unkown binary packages take <secs> seconds to merge]:secs:_default' \
'-W+[Maximum width of emerge proces commandline (default 60)]:num:_default' \
'--pwidth=[Maximum width of emerge proces commandline (default 60)]:num:_default' \
'-D+[Maximum depth of emerge proces tree (default 3)]:num:_default' \
'--pdepth=[Maximum depth of emerge proces tree (default 3)]:num:_default' \
'--avg=[Select function used to predict durations]:fn:_default' \
'--limit=[Use the last <num> merge times to predict durations]:num:_default' \
'-f+[Only parse log entries after <date/command>]:date:_default' \
'--from=[Only parse log entries after <date/command>]:date:_default' \
'-t+[Only parse log entries before <date/command>]:date:_default' \
'--to=[Only parse log entries before <date/command>]:date:_default' \
'-H+[Show table header]::bool:_default' \
'--header=[Show table header]::bool:_default' \
'--duration=[Output durations in different formats]:format:_default' \
'--date=[Output dates in different formats]:format:_default' \
'--utc=[Parse/display dates in UTC instead of local time]::bool:_default' \
'--color=[Enable color (yes/no/auto)]::bool:_default' \
'-o+[Output format (columns/tab/auto)]:format:_default' \
'--output=[Output format (columns/tab/auto)]:format:_default' \
'-F+[Location of emerge log file]:file:_default' \
'--logfile=[Location of emerge log file]:file:_default' \
'-S+[Show number of skipped rows (yes/no)]::bool:_default' \
'--showskip=[Show number of skipped rows (yes/no)]::bool:_default' \
'--theme=[Set terminal colors]:key_SGR:_default' \
'--tty=[Assume stdin/stdout is a terminal (in/out/inout/none/auto)]::inout:(auto in out inout none)' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]'
                ;;
            (stats)
                _arguments -s -S -C \
'-s+[Show emerge (r)uns, (p)ackages, (t)otals, (s)yncs, and/or (a)ll]:r,p,t,s,a:_default' \
'--show=[Show emerge (r)uns, (p)ackages, (t)otals, (s)yncs, and/or (a)ll]:r,p,t,s,a:_default' \
'-g+[Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one]:y,m,w,d,n:_default' \
'--groupby=[Group by (y)ear, (m)onth, (w)eek, (d)ay, (n)one]:y,m,w,d,n:_default' \
'--mtimedbfile=[Location of portage mtimedb file]:file:_default' \
'--avg=[Select function used to predict durations]:fn:_default' \
'--limit=[Use the last <num> merge times to predict durations]:num:_default' \
'-f+[Only parse log entries after <date/command>]:date:_default' \
'--from=[Only parse log entries after <date/command>]:date:_default' \
'-t+[Only parse log entries before <date/command>]:date:_default' \
'--to=[Only parse log entries before <date/command>]:date:_default' \
'-H+[Show table header]::bool:_default' \
'--header=[Show table header]::bool:_default' \
'--duration=[Output durations in different formats]:format:_default' \
'--date=[Output dates in different formats]:format:_default' \
'--utc=[Parse/display dates in UTC instead of local time]::bool:_default' \
'--color=[Enable color (yes/no/auto)]::bool:_default' \
'-o+[Output format (columns/tab/auto)]:format:_default' \
'--output=[Output format (columns/tab/auto)]:format:_default' \
'-F+[Location of emerge log file]:file:_default' \
'--logfile=[Location of emerge log file]:file:_default' \
'-S+[Show number of skipped rows (yes/no)]::bool:_default' \
'--showskip=[Show number of skipped rows (yes/no)]::bool:_default' \
'--theme=[Set terminal colors]:key_SGR:_default' \
'--tty=[Assume stdin/stdout is a terminal (in/out/inout/none/auto)]::inout:(auto in out inout none)' \
'-e[Match <search> using plain string]' \
'--exact[Match <search> using plain string]' \
'*-v[Increase verbosity (can be given multiple times)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::search:($(emlop complete))'
                ;;
            (accuracy)
                _arguments -s -S -C \
'-s+[Show (m)erges, (p)ackages, (t)otal, and/or (a)ll]:m,p,t,a:_default' \
'--show=[Show (m)erges, (p)ackages, (t)otal, and/or (a)ll]:m,p,t,a:_default' \
'-N+[Show only the first <num> entries]::num:_default' \
'--first=[Show only the first <num> entries]::num:_default' \
'-n+[Show only the last <num> entries]::num:_default' \
'--last=[Show only the last <num> entries]::num:_default' \
'--avg=[Select function used to predict durations]:fn:_default' \
'--limit=[Use the last <num> merge times to predict durations]:num:_default' \
'-f+[Only parse log entries after <date/command>]:date:_default' \
'--from=[Only parse log entries after <date/command>]:date:_default' \
'-t+[Only parse log entries before <date/command>]:date:_default' \
'--to=[Only parse log entries before <date/command>]:date:_default' \
'-H+[Show table header]::bool:_default' \
'--header=[Show table header]::bool:_default' \
'--duration=[Output durations in different formats]:format:_default' \
'--date=[Output dates in different formats]:format:_default' \
'--utc=[Parse/display dates in UTC instead of local time]::bool:_default' \
'--color=[Enable color (yes/no/auto)]::bool:_default' \
'-o+[Output format (columns/tab/auto)]:format:_default' \
'--output=[Output format (columns/tab/auto)]:format:_default' \
'-F+[Location of emerge log file]:file:_default' \
'--logfile=[Location of emerge log file]:file:_default' \
'-S+[Show number of skipped rows (yes/no)]::bool:_default' \
'--showskip=[Show number of skipped rows (yes/no)]::bool:_default' \
'--theme=[Set terminal colors]:key_SGR:_default' \
'--tty=[Assume stdin/stdout is a terminal (in/out/inout/none/auto)]::inout:(auto in out inout none)' \
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
