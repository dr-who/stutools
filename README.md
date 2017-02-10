# stutools
Stu's handy tools

./readSpeedTest  -- reads from N disks using N threads

  Options: -I (4096 blocks instead of 1M blocks)
           -r (random seeks between IO commands)

./writeSpeedTest -- writes to N disks using N threads

./writePairTest  -- writes to all combinations of 2 disks in 2 threads, plus analysis


Usage: ./writePairTest -F /dev/sd[cdefghijklmnop]

 
