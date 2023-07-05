#!/bin/bash

for a in rw
do
    
    for k in 4 8 16 32 64 1024
    do
	for r in data-ss-iops/bb-${a}-*-${k}
	do
	    cat $r | tail -n +2 | awk '{print $4+$6}' | ../stat median
	done | awk '{print NR, $1}' > bs_${k}
    done 
done

echo "set xtics 1" > out.gnuplot
echo "set key outside" >> out.gnuplot
echo "set key tmargin center horiz" >> out.gnuplot
echo "set yrange [0:]" >>out.gnuplot
echo "set xlabel 'Rounds'" >> out.gnuplot
echo "set ylabel 'IOPS'" >> out.gnuplot
echo "set grid" >> out.gnuplot
echo "set term svg" >> out.gnuplot
echo "set output 'ss-iops.svg'" >> out.gnuplot
echo "set title 'spit: IOPS Steady State Convergence Report'" >> out.gnuplot
#echo "plot 'bs_0.5' with linespoints, 'bs_4' with linespoints, 'bs_8' with linespoints, 'bs_16.linespoints', 'bs_32' with linespoints, 'bs_64' with linespoints, 'bs_1024' with linespoints" >> out.gnuplot
echo "plot 'bs_4' with linespoints title '4 KiB', 'bs_8' with linespoints title '8 KiB' , 'bs_16' with linespoints title '16 KiB', 'bs_32' with linespoints title '32 KiB', 'bs_64' with linespoints title '64 KiB', 'bs_1024' with linespoints title '1024 KiB'" >> out.gnuplot




