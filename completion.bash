_emlop() {
    local i cur prev opts cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd="emlop"
    opts=""

    i=0
    for w in ${COMP_WORDS[@]}; do
        found=$(compgen -W 'accuracy log predict stats' -- "${w}")
        case ${found} in
            "")
                ;;
            *)
                if [[ $i == $COMP_CWORD ]]; then
                    COMPREPLY=($found)
                    return 0
                else
                    cmd="emlop__$found"
                    break
                fi
                ;;
        esac
        let i=$i+1
    done

    case "${cmd}" in
        emlop)
            opts="log predict stats accuracy -f -t -H -o -F -v -h -V --from --to --header --showskip --duration --date --utc --color --theme --output --logfile --help --version"
            if [[ ${cur} == -* ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from|--to|-f|-t)
                    COMPREPLY=($(compgen -W "1h 1d 1w 1m 1h $(date -Is)" "${cur}"))
                    ;;
                --header|-H|--showskip)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --duration)
                    COMPREPLY=($(compgen -W "hms secs hmsfixed human" "${cur}"))
                    ;;
                --date)
                    COMPREPLY=($(compgen -W "ymd ymdhms ymdhmso rfc3339 rfc2822 compact unix" "${cur}"))
                    ;;
                --utc)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "yes no auto" "${cur}"))
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "merge: binmerge: unmerge: sync: duration: qmark: skip:" "${cur}"))
                    ;;
                --output|-o)
                    COMPREPLY=($(compgen -W "tab columns auto" "${cur}"))
                    ;;
                --logfile|-F)
                    COMPREPLY=($(compgen -f "${cur}"))
                    ;;
                *)
                    COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
                    ;;
            esac
            return 0
            ;;
        emlop__accuracy)
            opts="[search]... -e -s -n -f -t -H -o -F -v -h --exact --show --last --avg --limit --from --to --header --showskip --duration --date --utc --color --theme --output --logfile --help"
            if [[ ${cur} == -* ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from|--to|-f|-t)
                    COMPREPLY=($(compgen -W "1h 1d 1w 1m 1h $(date -Is)" "${cur}"))
                    ;;
                --header|-H|--showskip)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --duration)
                    COMPREPLY=($(compgen -W "hms secs hmsfixed human" "${cur}"))
                    ;;
                --date)
                    COMPREPLY=($(compgen -W "ymd ymdhms ymdhmso rfc3339 rfc2822 compact unix" "${cur}"))
                    ;;
                --utc)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "yes no auto" "${cur}"))
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "merge: binmerge: unmerge: sync: duration: qmark: skip:" "${cur}"))
                    ;;
                --output|-o)
                    COMPREPLY=($(compgen -W "tab columns auto" "${cur}"))
                    ;;
                --logfile|-F)
                    COMPREPLY=($(compgen -f "${cur}"))
                    ;;
                --show|-s)
                    COMPREPLY=($(compgen -W "mta" "${cur}"))
                    ;;
                --last|-n)
                    COMPREPLY=($(compgen -W "1 5 10 20 100" "${cur}"))
                    ;;
                --avg)
                    COMPREPLY=($(compgen -W "arith median weighted-arith weighted-median" "${cur}"))
                    ;;
                --limit)
                    COMPREPLY=($(compgen -W "1 5 20 999" "${cur}"))
                    ;;
                *)
                    if [[ -z "${cur}" ]]; then
                        COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
                    else
                        COMPREPLY=($(emlop complete -- "${cur}"))
                    fi
                    ;;
            esac
            return 0
            ;;
        emlop__log)
            opts="[search]... -N -n -s -e -f -t -H -o -F -v -h --starttime --first --last --show --exact --from --to --header --showskip --duration --date --utc --color --theme --output --logfile --help"
            if [[ ${cur} == -* ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from|--to|-f|-t)
                    COMPREPLY=($(compgen -W "1h 1d 1w 1m 1h $(date -Is)" "${cur}"))
                    ;;
                --header|-H|--showskip)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --duration)
                    COMPREPLY=($(compgen -W "hms secs hmsfixed human" "${cur}"))
                    ;;
                --date)
                    COMPREPLY=($(compgen -W "ymd ymdhms ymdhmso rfc3339 rfc2822 compact unix" "${cur}"))
                    ;;
                --utc)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "yes no auto" "${cur}"))
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "merge: binmerge: unmerge: sync: duration: qmark: skip:" "${cur}"))
                    ;;
                --output|-o)
                    COMPREPLY=($(compgen -W "tab columns auto" "${cur}"))
                    ;;
                --logfile|-F)
                    COMPREPLY=($(compgen -f "${cur}"))
                    ;;
                --starttime)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --first|-N|--last|-n)
                    COMPREPLY=($(compgen -W "1 5 10 20 100" "${cur}"))
                    ;;
                --show|-s)
                    COMPREPLY=($(compgen -W "musa" "${cur}"))
                    ;;
                *)
                    if [[ -z "${cur}" ]]; then
                        COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
                    else
                        COMPREPLY=($(emlop complete -- "${cur}"))
                    fi
                    ;;
            esac
            return 0
            ;;
        emlop__predict)
            opts="-s -N -n -W -D -f -t -H -o -F -v -h --show --first --last --tmpdir --resume --unknownc --unknownb --avg --limit --from --to --header --showskip --duration --date --utc --color --theme --output --pdepth --pwidth --logfile --help"
            if [[ ${cur} == -* ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from|--to|-f|-t)
                    COMPREPLY=($(compgen -W "1h 1d 1w 1m 1h $(date -Is)" "${cur}"))
                    ;;
                --header|-H|--showskip)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --duration)
                    COMPREPLY=($(compgen -W "hms secs hmsfixed human" "${cur}"))
                    ;;
                --date)
                    COMPREPLY=($(compgen -W "ymd ymdhms ymdhmso rfc3339 rfc2822 compact unix" "${cur}"))
                    ;;
                --utc)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "yes no auto" "${cur}"))
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "merge: binmerge: unmerge: sync: duration: qmark: skip:" "${cur}"))
                    ;;
                --pwidth|-W)
                    COMPREPLY=($(compgen -W "10 20 40 80 160" "${cur}"))
                    ;;
                --pdepth|-D)
                    COMPREPLY=($(compgen -W "0 1 3 5 7 99" "${cur}"))
                    ;;
                --output|-o)
                    COMPREPLY=($(compgen -W "tab columns auto" "${cur}"))
                    ;;
                --logfile|-F)
                    COMPREPLY=($(compgen -f "${cur}"))
                    ;;
                --show|-s)
                    COMPREPLY=($(compgen -W "emta" "${cur}"))
                    ;;
                --first|-N|--last|-n)
                    COMPREPLY=($(compgen -W "1 5 10 20 100" "${cur}"))
                    ;;
                --tmpdir)
                    COMPREPLY=($(compgen -d "${cur}"))
                    ;;
                --resume)
                    COMPREPLY=($(compgen -W "auto either main backup no" -- "${cur}"))
                    ;;
                --unknownc|--unknownb)
                    COMPREPLY=($(compgen -W "0 5 10 20 60" "${cur}"))
                    ;;
                --avg)
                    COMPREPLY=($(compgen -W "arith median weighted-arith weighted-median" "${cur}"))
                    ;;
                --limit)
                    COMPREPLY=($(compgen -W "1 5 20 999" "${cur}"))
                    ;;
                *)
                    COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
                    ;;
            esac
            return 0
            ;;
        emlop__stats)
            opts="[search]... -s -g -e -f -t -H -o -F -v -h --show --groupby --exact --avg --limit --from --to --header --showskip --duration --date --utc --color --theme --output --logfile --help"
            if [[ ${cur} == -* ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from|--to|-f|-t)
                    COMPREPLY=($(compgen -W "1h 1d 1w 1m 1h $(date -Is)" "${cur}"))
                    ;;
                --header|-H|--showskip)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --duration)
                    COMPREPLY=($(compgen -W "hms secs hmsfixed human" "${cur}"))
                    ;;
                --date)
                    COMPREPLY=($(compgen -W "ymd ymdhms ymdhmso rfc3339 rfc2822 compact unix" "${cur}"))
                    ;;
                --utc)
                    COMPREPLY=($(compgen -W "yes no" "${cur}"))
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "yes no auto" "${cur}"))
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "merge: binmerge: unmerge: sync: duration: qmark: skip:" "${cur}"))
                    ;;
                --output|-o)
                    COMPREPLY=($(compgen -W "tab columns auto" "${cur}"))
                    ;;
                --logfile|-F)
                    COMPREPLY=($(compgen -f "${cur}"))
                    ;;
                --show|-s)
                    COMPREPLY=($(compgen -W "ptsa" "${cur}"))
                    ;;
                --groupby|-g)
                    COMPREPLY=($(compgen -W "year month week day none" "${cur}"))
                    ;;
                --avg)
                    COMPREPLY=($(compgen -W "arith median weighted-arith weighted-median" "${cur}"))
                    ;;
                --limit)
                    COMPREPLY=($(compgen -W "1 5 20 999" "${cur}"))
                    ;;
                *)
                    if [[ -z "${cur}" ]]; then
                        COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
                    else
                        COMPREPLY=($(emlop complete -- "${cur}"))
                    fi
                    ;;
            esac
            return 0
            ;;
    esac
}

complete -F _emlop -o nosort -o bashdefault -o default emlop
