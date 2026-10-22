#!/bin/bash


for f in zx2s1n zx1s0 zx2s0 zx4s0 zx2s0n zx4s0 zs0nx2 zs1nx2 zs0nx4 zs1wnx2 zws1x1 zws1x2= zws1x4= zws8x1 zws-8x1 zws-1x1 zs4x1 zws0 zwP10000x1 zwP.10000x1 zwP+10000x1 zwP-10000x1
do
    sudo ./spit -f wow -G 1 -c k128q1$f -R1 -P - >p

    sudo awk '{print $15,$2,$6}' < p  | tee pos-$f > 11

    echo "unset key" >pattern.gnu
    echo "unset xtics" >> pattern.gnu
    echo "unset ytics" >> pattern.gnu
    echo "set xlabel 'time'" >> pattern.gnu
    echo "set ylabel 'position'" >> pattern.gnu
    echo "set term png size 1024,768" >> pattern.gnu
    echo "set output \"${f}.png\"" >> pattern.gnu
    echo "plot '<cat 11 | grep R' using 1:2 with points lt rgb \"green\", '<cat 11 | grep W' using 1:2 with points lt rgb \"red\"" >> pattern.gnu
    gnuplot <pattern.gnu

    echo ${f}.png

done

for f in zs0j4G_ zs0j4 
do
    sudo ./spit -f wow -G 1 -c k128q1$f -R1 -P - >p

    sudo awk '{print $15,$2,$6,$11}' < p  | tee pos-$f > 11

    echo "unset key" >pattern.gnu
    echo "unset xtics" >> pattern.gnu
    echo "unset ytics" >> pattern.gnu
    echo "set xlabel 'time'" >> pattern.gnu
    echo "set ylabel 'position'" >> pattern.gnu
    echo "set term png size 1024,768" >> pattern.gnu
    echo "set output \"${f}.png\"" >> pattern.gnu
    echo "plot '<cat 11 | cut -d " " -f 11' using 1:2 with points lt rgb \"green\", '<cat 11 | cut -d " " -f 11' using 1:2 with points lt rgb \"red\", '<cat 11 | cut -d " " -f 11' using 1:2 with points lt rgb \"blue\", '<cat 11 | cut -d " " -f 11' using 1:2 with points lt rgb \"orange\"" >> pattern.gnu

    gnuplot <pattern.gnu

    echo ${f}.png
done

