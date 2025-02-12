#!/bin/bash

##################################################
# Check preconditions
##################################################
cd $(dirname $(realpath $0))/..

if [ ! $TERM = alacritty ]; then
    echo "Restart in alacritty terminal"
    exit 1
fi


echo "emerge -qO1 dummybuild"
echo "for i in /sys/devices/system/cpu/cpufreq/policy*/scaling_governor; do echo performance > $i; done"
echo "Resize terminal to 120x30"
while true; do
    C=$(tput cols)
    L=$(tput lines)
    P=$(emlop p -sm -ot|rg dummybuild|cut -f1)
    S=$(cat /sys/devices/system/cpu/cpufreq/policy0/scaling_governor)
    if [ $C = 120 -a $L = 30 -a -n "$P" -a "$S" = performance ]; then
        break
    else
        echo -ne "\remerge=$P cols=$C lines=$L governor=$S Fix your setup, see above"
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
./benches/exec_compare.rs -o benches/bench1.csv -r 50 e:./target/release/emlop q g -sl,ltmu,egcc,ls,c
# qlop doesn't implement pgcc,pqt,pkde
./benches/exec_compare.rs -o benches/bench2.csv -r 50 e:./target/release/emlop g -spgcc
# genlop is too slow for 50 iterations of pqt,pkde
./benches/exec_compare.rs -o benches/bench3.csv -r 50 e:./target/release/emlop -spqt,pkde
./benches/exec_compare.rs -o benches/bench4.csv -r 10 g -spqt
./benches/exec_compare.rs -o benches/bench5.csv -r 3 g -spkde


##################################################
# Output
##################################################
#git status
./target/release/emlop --version
qlop --version
genlop --version

cut -f1,2,4 benches/bench?.csv|rg -v '^\*'|sort
