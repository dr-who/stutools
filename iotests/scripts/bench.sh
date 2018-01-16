#!/bin/bash


if [ $# -le 1 ]
then
    echo "./bench.sh /dev/nbd0 *.cli"
    exit 1;
fi

logfile=logfile.$$
echo "Logfile: "$logfile

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
	# 255 is exit(-1) and the speed goes from 0.. upwards. 255 would be 25.5 GB/s, which is too high for now
	if [ $retcode -ne 255 ]
	   then
	       /bin/echo -e $line"\t"$retcode | awk -F'\t' '{printf "%4.1f GB/s\t%s\n", $2/10.0, $1}' 
	       
	       /bin/echo -e $line"\t"$retcode | tr -d ' ' | awk -F'\t' '{printf "%s\t%4.1f\n", $1, $2/10.0}' >> $logfile
	fi
	
	
    done < $filename 

done
