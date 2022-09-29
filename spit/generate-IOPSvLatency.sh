#!/bin/sh

/bin/rm -f IOPSvsLatency.txt
/bin/rm -f IOPSvsLatency-qd.txt

DEVICE=sdk

echo none >/sys/block/${DEVICE}/queue/scheduler
echo == nr_requests `cat /sys/block/${DEVICE}/queue/nr_requests` ==



MAXIOPS=1100
STEPS=$(echo $MAXIOPS | awk '{print $1/110}')

START=$STEPS
START=10

echo === $START $STEPS ====

while [ $START -le $MAXIOPS ]
do
    echo === $START ====
    spit -f /dev/${DEVICE} -c rs0q2048k4S${START} -B out.${START} -P pos.${START}

    START=$(echo $START $STEPS | awk '{print $1+$2}')

done

START=10

while [ $START -le $MAXIOPS ]
do
    echo == generate $START ====
    cat pos.${START} | awk '{print $14}' | ../utils/dist -a 10000 | awk -v s=${START} '{print s,$5,$4,$6}' | tail -n 1 >> IOPSvsLatency.txt
    cat out.${START} | awk -v s=${START} '{x+=$4} END {print s,x/NR}' >> IOPSvsLatency-qd.txt

    START=$(echo $START $STEPS | awk '{print $1+$2}')

done
