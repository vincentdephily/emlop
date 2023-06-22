#!/bin/bash

##################################################
# Check preconditions
##################################################
cd $(dirname $(realpath $0))

if [ ! $TERM = alacritty ]; then
    echo "Restart in alacritty terminal"
    exit 1
fi

while true; do
    C=$(tput cols)
    L=$(tput lines)
    P=$(emlop p -sm --tab|rg dummybuild|cut -f1)
    if [ $C = 120 -a $L = 30 -a -n "$P" ]; then
        break
    else
        echo -ne "\remerge=$P cols=$C lines=$L Resize terminal to 120x30 and/or start 'emerge -qO1 dummybuild'"
        sleep 0.5
    fi
done


##################################################
# Make sure we have a fresh build
##################################################
cargo build -r


##################################################
# Run the tests
##################################################
./benches/exec_compare.rs -o bench1.csv -r 50 -p e:./target/release/emlop,q,g -sl,ltmu,egcc,tgcc,c
# qlop doesn't implement pgcc,pqt,pkde
./benches/exec_compare.rs -o bench2.csv -r 50 -p e:./target/release/emlop,g -spgcc
# genlop is too slow for 50 iterations of pqt,pkde
./benches/exec_compare.rs -o bench3.csv -r 50 -p e:./target/release/emlop -spqt,pkde
./benches/exec_compare.rs -o bench4.csv -r 10 -p g -spqt
./benches/exec_compare.rs -o bench5.csv -r 3 -p g -spkde


##################################################
# Output
##################################################
#git status
./target/release/emlop --version
qlop --version
genlop --version

cut -f1,2,4 bench?.csv|rg -v '^\*'|sort