#!/bin/bash

set -e
cd $(dirname $(which $0))

echo "Generating all ebuild category/names in $PWD/catname.txt"
find /usr/portage -name '*.ebuild'|sed -r s/.ebuild$//|cut -d/ -f4,5|sort -u > catname.txt

echo "Generating an emerge log of all ebuilds all versions in $PWD/emerge.all.log"
rm emerge.all.log
d=$(date -d '2017-01-01T00:00:00Z' +%s)
for i in $(find /usr/portage -name '*.ebuild'|sed -r s/.ebuild$//|cut -d/ -f4,6|sort -u); do
    echo "$d: >>> emerge (1 of 1) $i to /" >> emerge.all.log
    let d++
    echo "$d::: completed emerge (1 of 7) $i to /" >> emerge.all.log
    let d++
done
