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

BLOCKSIZEK=1024

echo none >/sys/block/${TESTDEVICE}/queue/scheduler
echo == nr_requests `cat /sys/block/${TESTDEVICE}/queue/nr_requests` ==

spit -c rs0q2048k${BLOCKSIZEK} -f /dev/${TESTDEVICE} -B maxspeed -t 20

MAXIOPS=$(tail -n +3 maxspeed | gawk '{x+=$4} END {print int(1.5*x/NR)}')

STEPS=$(echo $MAXIOPS | awk '{print int($1*1.0/20)}')
echo === processing block device $TESTDEVICE in steps of $STEPS to $MAXIOPS


/bin/rm -f IOPSvsLatency.txt
/bin/rm -f IOPSvsLatency-qd.txt



START=$STEPS

echo === $START $STEPS ====

while [ $START -le $MAXIOPS ]
do
    echo === $START ====
    spit -f /dev/${TESTDEVICE} -c rs0q2048k${BLOCKSIZEK}S${START} -B out.${START} -P pos.${START}

    START=$(echo $START $STEPS | awk '{print $1+$2}')

done

START=$STEPS

while [ $START -le $MAXIOPS ]
do
    echo == generate $START ====
    cat pos.${START} | tail -n +2 | awk '{print $14}' | ../utils/dist -a 10000 | awk -v s=${START} '{print s,1000.0*$5,1000.0*$4,1000.0*$6}' | tail -n 1 >> IOPSvsLatency.txt
    cat out.${START} | awk -v s=${START} '{x+=$4} END {print s,1.0*x/NR}' >> IOPSvsLatency-qd.txt

    START=$(echo $START $STEPS | awk '{print $1+$2}')

done


echo "set xlabel 'Target I/O Operations per Second (IOPS)'" > IOPSvsLatency.gnu
echo "set ylabel 'Average latency (ms)'" >> IOPSvsLatency.gnu
echo "set y2label 'Achieved I/O Operations per Second (IOPS)'" >> IOPSvsLatency.gnu
echo "set title 'Random read ${BLOCKSIZEK}KiB operations'" >> IOPSvsLatency.gnu
echo "set ytics out" >> IOPSvsLatency.gnu
echo "set xtics out" >> IOPSvsLatency.gnu
echo "set y2tics out" >> IOPSvsLatency.gnu
echo "set y2range [0:]" >> IOPSvsLatency.gnu
echo "set log y" >> IOPSvsLatency.gnu
echo "set key outside top center horiz" >> IOPSvsLatency.gnu
echo "plot  'IOPSvsLatency.txt' with yerror title 'Latency s.d. (ms)', 'IOPSvsLatency.txt' using 1:2 with lines title 'Mean latency (ms)', 'IOPSvsLatency-qd.txt' using 1:2 axes x1y2 with lines title 'Achieved IOPS'" >> IOPSvsLatency.gnu



