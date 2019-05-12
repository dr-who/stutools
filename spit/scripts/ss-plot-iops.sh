#!/bin/bash

for a in w
do
    
    for k in 4 8 16 32 64 1024
    do
	for r in data-ss-iops/bb-${a}-*-${k}-000 
	do
	    cat $r | tail -n 60 | awk '{print $5}' | ../../utils/median
	done | awk '{print NR, $1}' > bs_${k}
    done 
done

echo "set xtics 1" > out.gnuplot
echo "set key outside" >> out.gnuplot
echo "set yrange [0:]" >>out.gnuplot
echo "set xlabel 'rounds'" >> out.gnuplot
echo "set ylabel 'IOPS'" >> out.gnuplot
echo "set grid" >> out.gnuplot
echo "set term svg" >> out.gnuplot
echo "set output 'ss-iops.svg'" >> out.gnuplot
echo "set title 'spit: IOPS Steady State Convergence Report'" >> out.gnuplot
#echo "plot 'bs_0.5' with linespoints, 'bs_4' with linespoints, 'bs_8' with linespoints, 'bs_16.linespoints', 'bs_32' with linespoints, 'bs_64' with linespoints, 'bs_1024' with linespoints" >> out.gnuplot
echo "plot 'bs_4' with linespoints, 'bs_8' with linespoints, 'bs_16' with linespoints, 'bs_32' with linespoints, 'bs_64' with linespoints, 'bs_1024' with linespoints" >> out.gnuplot




