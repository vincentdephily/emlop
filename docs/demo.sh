#!/bin/bash

function usage() {
    echo 1>&2 "Error: $@

This script helps creating screencasts for the readme

Usage:
    cd <output folder>
    $0 <*.asc files>

Requirements:
    emerge asciinema libwebp
    git clone https://github.com/zechris/asciinema-rec_script && add to path
    cargo install agg --git https://github.com/asciinema/agg
    cargo install emlop --path ."
    exit 2
}

#[ -z "$1" ] && usage "No file given"
cd $(dirname $(realpath $0))
export EMLOP_CONFIG=$(pwd)/emlop.demo.toml
if [ -n "$@" ]; then
    for SCN in $@; do
        rg "^[a-z]+.asc$" <<< "$SCN" || usage "Expected asc file name, got '$SCN'"
        SCN=${SCN%.asc}
        echo "========== reccord $SCN =========="
        COMMENT="[3m[32m# " CAT=cat PROMPT_PAUSE=0 TYPING_PAUSE=0.06 time ./$SCN.asc || usage "$SCN.asc failed"
        sed -i /recording/d $SCN.cast
    done
fi

for SCN in *.cast; do
    SCN=${SCN%.cast}
    if [ $SCN.cast -nt $SCN.gif -o ! -e $SCN.gif ]; then
        echo "========== agg $SCN =========="
        # Default idle-limit is lower than documented and need to be raised
        # fps-cap significantly reduces the final file size
        agg --cols 106 --rows 20 --idle-time-limit 10 --fps-cap 10 --last-frame-duration 5 $SCN.cast $SCN.gif || usage "agg failed"
    fi
    if [ $SCN.gif -nt $SCN.webp -o ! -e $SCN.webp ]; then
        echo "========== gif2webp $SCN =========="
        gif2webp $SCN.gif -o $SCN.webp || usage "gif2webp failed"
    fi
done
