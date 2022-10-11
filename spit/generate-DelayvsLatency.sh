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

export BLOCKSIZEK=1024
export MODEL=$(smartctl -a /dev/${TESTDEVICE} | egrep "Model:|Product:" | head -n 1 | sed 's/.*: *//g')
export SIZETB=$(lsblk -b /dev/${TESTDEVICE} | tail -n 1 | awk '{printf "%.0lf\n",$4/1000000000000.0}')

echo === ${MODEL} - ${SIZETB}

echo none >/sys/block/${TESTDEVICE}/queue/scheduler
export MAXNR=$(cat /sys/block/${TESTDEVICE}/queue/nr_requests)

echo == nr_requests ${MAXNR} ==

export RANGE="0.05 0.1 0.2 0.3 0.4 0.5 1 1.5 2 2.5 3.33 5 10 50 100 150 200"
#export RANGE="0.5 1 1.5 2 2.5 3.33 5 10 50 100 150 200"

for IOPS in ${RANGE}
do
    echo === ${IOPS}
    RUNTIME=$(echo $IOPS | awk '{x=50.0/$1 + 2; if (x<20) x=20;} END {print x}')
    ./spit -f /dev/${TESTDEVICE} -c rs0q1k${BLOCKSIZEK}S${IOPS} -B ${TEMPDATA}/outiops.${IOPS} -P ${TEMPDATA}/posiops.${IOPS} -t ${RUNTIME}
done

#export RANGE="0.05 0.1 0.2 0.3 0.4 0.5 1 1.5 2 2.5 3.33 5 10 50 100 150 200"

/bin/rm -f DelayvsLatency.txt


for IOPS in ${RANGE}
do
    cat ${TEMPDATA}/posiops.${IOPS} | tail -n +2 | awk '{print 1000.0*$14}' | ./stat | awk -v s=${IOPS} '{print 1.0/s,$8,$6,$10}' | tail -n 1 >> DelayvsLatency.txt
done

XMIN=$(cat DelayvsLatency.txt | awk '{print $1}' | ./stat min | awk '{print $1*0.95}')
XMAX=$(cat DelayvsLatency.txt | awk '{print $1}' | ./stat max | awk '{print $1*1.05}')
YMIN=$(cat DelayvsLatency.txt | awk '{for (i=2;i<=NF;i++) print $i}' | ./stat  | awk '{print $4*0.95}')
YMAX=$(cat DelayvsLatency.txt | awk '{for (i=2;i<=NF;i++) print $i}' | ./stat  | awk '{print $12*1.05}')

echo "set xlabel 'Seconds between IOs'" > DelayvsLatency.gnu
echo "set ylabel 'Average latency (ms)'" >> DelayvsLatency.gnu
echo "set title 'Random read ${BLOCKSIZEK}KiB operations - ${MODEL} - ${SIZETB}TB'" >> DelayvsLatency.gnu
echo "set ytics out" >> DelayvsLatency.gnu
echo "set xtics out" >> DelayvsLatency.gnu
echo "set xrange [${XMIN}:${XMAX}]" >> DelayvsLatency.gnu
echo "set yrange [${YMIN}:${YMAX}]" >> DelayvsLatency.gnu
echo "set grid" >> DelayvsLatency.gnu
echo "set log x" >> DelayvsLatency.gnu
echo "set key outside top center horiz" >> DelayvsLatency.gnu
echo "plot  'DelayvsLatency.txt' with yerror title 'Latency s.d. (ms)', 'DelayvsLatency.txt' using 1:2 with lines title 'Mean latency (ms)'" >> DelayvsLatency.gnu


