#!/bin/bash

for k in 0.5 4 8 16 32 64 1024
do
    for r in 1 2 3 4 5
    do
	cat data-ss-iops/bb-${r}-${k}-000 | tail -n 60 | awk '{x+=$5} END {print x/NR}'
    done | awk '{print NR, $1}' > bs_${k}
done 

echo "set xtics 1" > out.gnuplot
echo "set key outside" >> out.gnuplot
echo "set yrange [0:]" >>out.gnuplot
echo "set xlabel 'rounds'" >> out.gnuplot
echo "set ylabel 'IOPS'" >> out.gnuplot
echo "set title 'IOPS SS Convergence Report'" >> out.gnuplot
echo "plot 'bs_0.5' with linespoints, 'bs_4' with linespoints, 'bs_8' with linespoints, 'bs_16.linespoints', 'bs_32' with linespoints, 'bs_64' with linespoints, 'bs_1024' with linespoints" >> out.gnuplot




