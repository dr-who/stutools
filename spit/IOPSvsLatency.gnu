set xlabel 'Target I/O Operations per Second (IOPS)'
set ylabel 'Average latency (ms)'
set y2label 'Achieved I/O Operations per Second (IOPS)'
set title 'Random read 4k operations'
set key outside top center horiz
plot  'IOPSvsLatency.txt' with yerror title 'Latency s.d. (ms)', 'IOPSvsLatency.txt' using 1:2 with lines title 'Mean latency (ms)', 'IOPSvsLatency-qd.txt' using 1:2 axes x1y2 with lines title 'Achieved IOPS'
