#!/bin/bash

DEV=$1

if [ "$DEV" = "" ]; then echo missing device; exit 1; fi

SPIT=../../spit

#ignores the first number
TIME=6

mkdir data-ss-iops
cd data-ss-iops


#${SPIT} -f ${DEV} -c wk64 -t 100

#${SPIT} -f ${DEV} -c ws0 -t 100

for a in r rwwwwwwwwwwwwwwwwwww rrrrrrrwwwwwwwwwwwww rw rrrrrrrrrrrrrwwwwwww rrrrrrrrrrrrrrrrrrrw w
do
    
    
    for k in 4 8 16 32 64 128 1024
    do
	for r in 1 2 3 4 5
	do
	    echo =============== $r $k
	    sync
	${SPIT} -f ${DEV} -c ${a}s0k${k}q128 -t${TIME} -B bb-$a-$r-$k
	done
    done
done

cd ..

