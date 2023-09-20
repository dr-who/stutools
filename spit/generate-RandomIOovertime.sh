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
echo === ${MODEL}

echo none >/sys/block/${TESTDEVICE}/queue/scheduler
export MAXNR=$(cat /sys/block/${TESTDEVICE}/queue/nr_requests)

echo == nr_requests ${MAXNR} ==


/bin/rm -f RandomIOovertime.txt

./spit -f /dev/${TESTDEVICE} -c rs0q${MAXNR}-${MAXNR}k${BLOCKSIZEK} -B RandomIOovertime.txt -P ${TEMPDATA}/pos.riot -t 120

#MIN=$(cat IOPSvsLatency.txt | awk '{for (i=2;i<=NF;i++) print $i}' | ./stat  | awk '{print $4}')
#MAX=$(cat IOPSvsLatency.txt | awk '{for (i=2;i<=NF;i++) print $i}' | ./stat  | awk '{print $12}')

echo "set term dumb 100,40" > RandomIOovertime.gnu
echo "set xlabel 'Time (s)'" >> RandomIOovertime.gnu
echo "set ylabel 'Average latency (ms)'" >> RandomIOovertime.gnu
echo "set title 'Random read ${BLOCKSIZEK} KiB operations - ${MODEL}'" >> RandomIOovertime.gnu
echo "set ytics out" >> RandomIOovertime.gnu
echo "set xtics out" >> RandomIOovertime.gnu
echo "set grid" >> RandomIOovertime.gnu
echo "set key outside top center horiz" >> RandomIOovertime.gnu
echo "plot  'tmpdata/pos.riot' using 12:14 notitle" >> RandomIOovertime.gnu

