#!/bin/bash

for f in Timevs*.gnu; do g=${f%%.gnu}; cat <(echo set term png size '1000,800' font \'arial,10\') <(echo set log y) <(echo set yrange \[0.001:0.4\]) <(echo set output \'$g.png\') <(echo load \'$g'.gnu'\')| gnuplot; done

