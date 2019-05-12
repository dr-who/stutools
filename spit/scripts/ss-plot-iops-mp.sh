#!/bin/bash

for a in r rwwwwwwwwwwwwwwwwwww rrrrrrrwwwwwwwwwwwww rw rrrrrrrrrrrrrwwwwwww rrrrrrrrrrrrrrrrrrrw w
do
    for k in 4 8 16 32 64 1024
    do
	cat data-ss-iops/bb-${a}-*-${k}-000 | tail -n 60 | awk '{print $5}' | ../../utils/median | awk -v b=${k} '{print b, $1}' 
    done > iops_mp_data.${a}
done 

echo "set xrange [1:10000]" > out.gnuplot
echo "set yrange [1:100000]" >> out.gnuplot
#echo "set key tmargin center horiz" >> out.gnuplot
echo "set key outside" >> out.gnuplot
echo "set grid" >> out.gnuplot
echo "set xlabel 'Block Size (KiB)'" >> out.gnuplot
echo "set ylabel 'IOPS'" >> out.gnuplot
echo "set title 'spit: IOPS Measurement Plot - 2D'" >> out.gnuplot
echo "set log x" >> out.gnuplot
echo "set log y" >> out.gnuplot
echo "set style line 1 pt 2" >> out.gnuplot
echo "set term svg" >> out.gnuplot
echo "set output 'iops-bs-2d.svg'" >> out.gnuplot
echo "plot 'iops_mp_data.r' with linespoints title '0/100', 'iops_mp_data.rwwwwwwwwwwwwwwwwwww' with linespoints title '5/95', 'iops_mp_data.rrrrrrrwwwwwwwwwwwww' with linespoints title '35/65', 'iops_mp_data.rw' with linespoints title '50/50', 'iops_mp_data.rrrrrrrrrrrrrwwwwwww' with linespoints title '65/35', 'iops_mp_data.rrrrrrrrrrrrrrrrrrrw' with linespoints title '95/5', 'iops_mp_data.w' with linespoints title '100/0' " >> out.gnuplot




cat iops_mp_data.r | awk '{print "100",$1,$2}' > d
cat iops_mp_data.rrrrrrrwwwwwwwwwwwww | awk '{print "35",$1,$2}' >> d
cat iops_mp_data.rrrrrrrrrrrrrwwwwwww | awk '{print "65",$1,$2}' >> d
cat iops_mp_data.w | awk '{print "100",$1,$2}' >> d
