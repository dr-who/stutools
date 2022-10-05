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

./spit -c rs0q${MAXNR}k${BLOCKSIZEK} -f /dev/${TESTDEVICE} -B maxspeed -t 20

export MAXIOPS=$(tail -n +3 maxspeed | gawk '{x+=$4} END {print int(1.5*x/NR)}')

export STEPS=$(echo $MAXIOPS | awk '{print int($1*1.0/40)}')
echo === $MODEL - processing block device $TESTDEVICE in steps of $STEPS to $MAXIOPS


/bin/rm -f IOPSvsLatency.txt
/bin/rm -f IOPSvsLatency-qd.txt



export START=$STEPS

echo === $START $STEPS ====

while [ $START -le $MAXIOPS ]
do
    echo === $START ====
    ./spit -f /dev/${TESTDEVICE} -c rs0q${MAXNR}k${BLOCKSIZEK}S${START} -B ${TEMPDATA}/out.${START} -P ${TEMPDATA}/pos.${START}

    START=$(echo $START $STEPS | awk '{print $1+$2}')

done

START=$STEPS

while [ $START -le $MAXIOPS ]
do
    echo == generate $START ====
    cat ${TEMPDATA}/pos.${START} | tail -n +2 | awk '{print 1000.0*$14}' | ./stat | awk -v s=${START} '{print s,$8,$6,$10}' | tail -n 1 >> IOPSvsLatency.txt
    cat ${TEMPDATA}/out.${START} | awk -v s=${START} '{x+=$4} END {print s,1.0*x/NR}' >> IOPSvsLatency-qd.txt

    START=$(echo $START $STEPS | awk '{print $1+$2}')

done

MIN=$(cat IOPSvsLatency.txt | awk '{for (i=2;i<=NF;i++) print $i}' | ./stat  | awk '{print $4}')
MAX=$(cat IOPSvsLatency.txt | awk '{for (i=2;i<=NF;i++) print $i}' | ./stat  | awk '{print $12}')

echo "set xlabel 'Target I/O Operations per Second (IOPS)'" > IOPSvsLatency.gnu
echo "set ylabel 'Average latency (ms)'" >> IOPSvsLatency.gnu
echo "set y2label 'Achieved I/O Operations per Second (IOPS)'" >> IOPSvsLatency.gnu
echo "set title 'Random read ${BLOCKSIZEK}KiB operations - ${MODEL}'" >> IOPSvsLatency.gnu
echo "set ytics out" >> IOPSvsLatency.gnu
echo "set xtics out" >> IOPSvsLatency.gnu
echo "set xrange [0:${MAXIOPS}]" >> IOPSvsLatency.gnu
echo "set yrange [${MIN}:${MAX}]" >> IOPSvsLatency.gnu
echo "set grid" >> IOPSvsLatency.gnu
echo "set y2tics out" >> IOPSvsLatency.gnu
echo "set y2range [0:${MAXIOPS}]" >> IOPSvsLatency.gnu
echo "set key outside top center horiz" >> IOPSvsLatency.gnu
echo "plot  'IOPSvsLatency.txt' with yerror title 'Latency s.d. (ms)', 'IOPSvsLatency.txt' using 1:2 with lines title 'Mean latency (ms)', 'IOPSvsLatency-qd.txt' using 1:2 axes x1y2 with lines title 'Achieved IOPS'" >> IOPSvsLatency.gnu

