#!/bin/bash

DEV=$1

if [ "$DEV" = "" ]; then echo missing device; exit 1; fi

SPIT=../../spit
TIME=5

mkdir data-ss-iops
cd data-ss-iops


${SPIT} -f ${DEV} -c wk64 -t 100

${SPIT} -f ${DEV} -c ws0 -t 100

for k in 0.5 4 8 16 32 64 128 1024
do
    for r in 1 2 3 4 5
    do
	echo =============== $r $k
	${SPIT} -f ${DEV} -c ws0k${k}q16 -t${TIME} -B bb-$r-$k
    done
done

cd ..

