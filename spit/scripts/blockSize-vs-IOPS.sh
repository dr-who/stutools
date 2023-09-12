#!/bin/bash

for a in r rwwwwwwwwwwwwwwwwwww rrrrrrrwwwwwwwwwwwww rw rrrrrrrrrrrrrwwwwwww rrrrrrrrrrrrrrrrrrrw w
do
    for k in 4 8 16 32 64 1024
    do
	cat data-ss-iops/bb-${a}-*-${k} | tail -n +2 | awk '{print $4 + $6}' | ../stat median | awk -v b=${k} '{print b, $1}' 
    done > iops_mp_data.${a}
done 

#echo "set xrange auto" > blockSize-vs-IOPS.gnu
#echo "set yrange auto" >> blockSize-vs-IOPS.gnu
echo "set term dumb 100,40" > blockSize-vs-IOPS.gnu
echo "set key outside" >> blockSize-vs-IOPS.gnu
echo "set yrange []" >> blockSize-vs-IOPS.gnu
echo "set key tmargin center horiz" >> blockSize-vs-IOPS.gnu
echo "set grid" >> blockSize-vs-IOPS.gnu
echo "set xlabel 'Block Size (KiB)'" >> blockSize-vs-IOPS.gnu
echo "set ylabel 'IOPS'" >> blockSize-vs-IOPS.gnu
echo "set title 'spit: IOPS Test'" >> blockSize-vs-IOPS.gnu
echo "set log x" >> blockSize-vs-IOPS.gnu
echo "set xtics 2" >> blockSize-vs-IOPS.gnu
echo "set ytics 2" >> blockSize-vs-IOPS.gnu
echo "set log y" >> blockSize-vs-IOPS.gnu
echo "set style line 1 pt 2" >> blockSize-vs-IOPS.gnu
#echo "set term svg" >> blockSize-vs-IOPS.gnu
#echo "set output 'iops-bs-2d.svg'" >> blockSize-vs-IOPS.gnu
echo "plot 'iops_mp_data.r' with linespoints title '100/0', 'iops_mp_data.rwwwwwwwwwwwwwwwwwww' with linespoints title '5/95', 'iops_mp_data.rrrrrrrwwwwwwwwwwwww' with linespoints title '35/65', 'iops_mp_data.rw' with linespoints title '50/50', 'iops_mp_data.rrrrrrrrrrrrrwwwwwww' with linespoints title '65/35', 'iops_mp_data.rrrrrrrrrrrrrrrrrrrw' with linespoints title '95/5', 'iops_mp_data.w' with linespoints title '0/100' " >> blockSize-vs-IOPS.gnu




cat iops_mp_data.r | awk '{print "100",$1,$2}' > d
cat iops_mp_data.rrrrrrrwwwwwwwwwwwww | awk '{print "35",$1,$2}' >> d
cat iops_mp_data.rrrrrrrrrrrrrwwwwwww | awk '{print "65",$1,$2}' >> d
cat iops_mp_data.w | awk '{print "100",$1,$2}' >> d


gnuplot 'blockSize-vs-IOPS.gnu'
