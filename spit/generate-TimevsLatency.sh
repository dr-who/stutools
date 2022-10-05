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

export RWMODE=w

export IOPS=100
export BLOCKSIZEK=2048
export SAMPLES=9
export MODEL=$(smartctl -a /dev/${TESTDEVICE} | egrep "Model:|Product:" | head -n 1 | sed 's/.*: *//g')
export SIZETB=$(lsblk -b /dev/${TESTDEVICE} | tail -n 1 | awk '{printf "%.0lf\n",$4/1000000000000.0}')

echo === ${MODEL} ${BLOCKSIZEK} ${SIZETB}

echo none >/sys/block/${TESTDEVICE}/queue/scheduler
export MAXNR=$(cat /sys/block/${TESTDEVICE}/queue/nr_requests)

echo == nr_requests ${MAXNR} ==

./spit -c z${RWMODE}s1q16k${BLOCKSIZEK}S${IOPS}X${SAMPLES} -f /dev/${TESTDEVICE} -P pos 10 -G 20

awk '{print $2,$14,$17,$18}' pos > TimevsLatency.txt

echo "set xlabel 'Disk position (MiB)'" > TimevsLatency.gnu
echo "set ylabel 'Latency (ms)'" >> TimevsLatency.gnu
echo "set title 'Seq ${RWMODE} - ${BLOCKSIZEK}KiB operations - ${MODEL} - ${SIZETB} TB'" >> TimevsLatency.gnu
echo "set ytics out" >> TimevsLatency.gnu
echo "set xtics out" >> TimevsLatency.gnu
echo "set grid" >> TimevsLatency.gnu
echo "set key outside top center horiz" >> TimevsLatency.gnu
echo "plot  'TimevsLatency.txt' using 1:2 with lines title 'mean latency', 'TimevsLatency.txt' using 1:3 with lines title 'median latency (N=${SAMPLES})', 'TimevsLatency.txt' using 1:4 with points title 'max latency'" >> TimevsLatency.gnu

