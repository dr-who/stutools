#!/bin/sh


if [ $# -le 1 ]
then
    echo "./run.sh /dev/loop3 *.cli"
    exit 1;
fi

time=2

device=$1
shift

for f in $*
do

    filename=$f
    echo === $device -- $f ===
    
    while read -r line
    do
	../aioRWTest -t$time -f $device $line >/dev/null 2>&1
	retcode=$?
	echo $line"\t"$retcode | awk -F'\t' '{printf "%4.1f GB/s\t%s\n", $2/10.0, $1}'
	
    done < $filename 

done
