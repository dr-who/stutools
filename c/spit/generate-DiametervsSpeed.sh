#!/bin/sh

if [ "$TESTDEVICE" = "" ]
then
    echo You need to set TESTDEVICE, e.g. export TESTDEVICE=sdk
    exit 1
fi

if [ ! -b "/dev/${TESTDEVICE}" ]
then
    echo "I couldn't find a directory called /dev/${TESTDEVICE}"
    exit 1
fi

TEMPDATA=tmpdata
mkdir -p ${TEMPDATA}

export IOPS=10
export BLOCKSIZEK=1024
export SAMPLES=7
export MODEL=$(smartctl -a /dev/${TESTDEVICE} | egrep "Model:|Product:" | head -n 1 | sed 's/.*: *//g')
export SIZETB=$(lsblk -b /dev/${TESTDEVICE} | tail -n 1 | awk '{printf "%.0lf\n",$4/1000000000000.0}')

echo === ${MODEL} ${BLOCKSIZEK} ${SIZETB}

echo none >/sys/block/${TESTDEVICE}/queue/scheduler
export MAXNR=$(cat /sys/block/${TESTDEVICE}/queue/nr_requests)

echo == nr_requests ${MAXNR} ==

STEPS=5

START=0
GAP=1

/bin/rm -f DiametervsSpeed.txt

while [ $START -le 100 ]
do
    echo === $START $GAP  step $STEPS ===

    if [ $START -eq 100 ]; then START=99; fi

    NEXT=$(echo $START $GAP| awk '{n=$1+$2; if (n>100) n=100;} END {print n}')
    
    ./spit -c zrs1q4k1024 -f /dev/${TESTDEVICE} -B out.${START} -P pos.${START} -G ${START}per-${NEXT}per -t 5

    MEDIAN=$(cat out.${START} | tail -n +2 | awk '{print $4}' | ./stat median min max)
    echo $START $MEDIAN >> DiametervsSpeed.txt
    
    START=$(echo $START $STEPS | awk '{print $1+$2}')
done


exit 0


awk '{print NR,$14,$17,$18}' pos > TimevsLatency.txt

echo "set term dumb 100,40" > TimevsLatency.gnu
echo "set xlabel 'Time (x)'" >> TimevsLatency.gnu
echo "set ylabel 'Latency (ms)'" >> TimevsLatency.gnu
echo "set title 'Seq read ${BLOCKSIZEK}KiB operations - ${MODEL} - ${SIZETB} TB'" >> TimevsLatency.gnu
echo "set ytics out" >> TimevsLatency.gnu
echo "set xtics out" >> TimevsLatency.gnu
echo "set grid" >> TimevsLatency.gnu
echo "set key outside top center horiz" >> TimevsLatency.gnu
echo "plot  'TimevsLatency.txt' using 1:2 with lines title 'mean latency', 'TimevsLatency.txt' using 1:3 with lines title 'median latency (N=${SAMPLES})', 'TimevsLatency.txt' using 1:4 with points title 'max latency'" >> TimevsLatency.gnu

