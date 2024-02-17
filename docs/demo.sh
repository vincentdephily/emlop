#!/bin/bash

function usage() {
    echo 1>&2 "Error: $@

This script helps creating screencasts for the readme

Usage:
    cd <ouput folder>
    $0 <*.asc files>

Requirements:
    emerge asciinema libwebp
    git clone https://github.com/zechris/asciinema-rec_script && add to path
    cargo install agg --git https://github.com/asciinema/agg
    cargo install emlop --path ."
    exit 2
}

[ -z "$@" ] && usage "No file given"
cd $(dirname $(realpath $0))
export EMLOP_CONFIG=$(pwd)/emlop.demo.toml
for SCN in $@; do
    rg "^[a-z]+.asc$" <<< "$SCN" || usage "Expected asc file name, got '$SCN'"
    echo "========== $SCN =========="
    SCN=${SCN%.asc}
    COMMENT="[3m[32m# " CAT=cat PROMPT_PAUSE=0 TYPING_PAUSE=0.06 ./$SCN.asc || usage "$SCN.asc failed"
    sed -i /recording/d $SCN.cast
    agg --cols 106 --rows 20 $SCN.cast $SCN.gif || usage "agg failed"
    gif2webp $SCN.gif -o $SCN.webp || usage "gif2webp failed"
done
