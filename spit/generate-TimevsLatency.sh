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

if [ "${RWMODE}" = "" ]; then export RWMODE=r; fi
if [ "${IOPS}" = "" ]; then export IOPS=100; fi
if [ "${GBSIZE}" = "" ]; then export GBSIZE=100; fi
if [ "${SAMPLES}" = "" ]; then export SAMPLES=5; fi
if [ "${BLOCKSIZEK}" = "" ]; then export BLOCKSIZEK=1024; fi



export MODEL=$(smartctl -a /dev/${TESTDEVICE} | egrep "Model:|Product:" | head -n 1 | sed 's/.*: *//g')
export REV=$(smartctl -a /dev/${TESTDEVICE} | egrep "Revision" | head -n 1 | sed 's/.*: *//g')
export SIZETB=$(lsblk -b /dev/${TESTDEVICE} | tail -n 1 | gawk '{printf "%.0lf\n",$4/1000000000000.0}')

echo === ${MODEL} ${BLOCKSIZEK} ${SIZETB} ${REV} ${RWMODE} ${IOPS} ${SAMPLES}

echo none >/sys/block/${TESTDEVICE}/queue/scheduler
export MAXNR=$(cat /sys/block/${TESTDEVICE}/queue/nr_requests)

echo == nr_requests ${MAXNR} ==

export FILEOUT=${TESTDEVICE}-${RWMODE}-${IOPS}IOPS-${MODEL}-${REV}-${GBSIZE}GB-N${SAMPLES}-k${BLOCKSIZEK}


#// read a block, wake up drive
dd if=/dev/${TESTDEVICE} bs=4k of=/dev/null count=1 2>/dev/null

./spit -c z${RWMODE}s1q16k${BLOCKSIZEK}S${IOPS}X${SAMPLES} -f /dev/${TESTDEVICE} -P pos-${FILEOUT} 10 -G ${GBSIZE}

gawk '{print $2,$14,$17,$18}' pos-${FILEOUT} > TimevsLatency-${FILEOUT}.txt

echo "set xlabel 'Disk position (MiB)'" > TimevsLatency-${FILEOUT}.gnu
echo "set ylabel 'Latency (ms)'" >> TimevsLatency-${FILEOUT}.gnu
echo "set title 'Seq ${RWMODE} - ${BLOCKSIZEK}KiB operations - ${FILEOUT} - ${SIZETB} TB'" >> TimevsLatency-${FILEOUT}.gnu
echo "set ytics out" >> TimevsLatency-${FILEOUT}.gnu
echo "set xtics out" >> TimevsLatency-${FILEOUT}.gnu
echo "set grid" >> TimevsLatency-${FILEOUT}.gnu
echo "set key outside top center horiz" >> TimevsLatency-${FILEOUT}.gnu
echo "plot 'TimevsLatency-${FILEOUT}.txt' using 1:4 with points title 'max latency', 'TimevsLatency-${FILEOUT}.txt' using 1:2 with lines title 'mean latency', 'TimevsLatency-${FILEOUT}.txt' using 1:3 with lines title 'median latency (N=${SAMPLES})'" >> TimevsLatency-${FILEOUT}.gnu

