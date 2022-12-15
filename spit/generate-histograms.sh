#!/bin/bash

for MODE in r w
do
    
for f in sdi sdl sdk
do
    cat TimevsLatency-${f}-${MODE}-100*.txt | awk '{print 1000*$2}' | ./hist -b 0.5 -o hist-${f}-${MODE} >consistency-${f}-${MODE}.txt  2>&1
    gnuplot <hist-${f}-${MODE}.gnu
    cat TimevsLatency-${f}-${MODE}-100*.txt | awk '{print $2}' | ./stat > stats-${f}-${MODE}2
    cat TimevsLatency-${f}-${MODE}-100*.txt | awk '{print $3}' | ./stat > stats-${f}-${MODE}3
    cat TimevsLatency-${f}-${MODE}-100*.txt | awk '{print $4}' | ./stat > stats-${f}-${MODE}4
    join stats-${f}-${MODE}2 stats-${f}-${MODE}3 | join - stats-${f}-${MODE}4 | column -t > stats-${f}-${MODE}.txt
done

done


