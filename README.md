# stutools
Stu's handy tools

./readSpeedTest  -- reads from N disks using N threads

  Options: -I (4096 blocks instead of 1M blocks)
           -r (random seeks between IO commands)
	   -kN (N KB, e.g. -k4 is 4096 bytes. -I is a shortcut for -k4)
	   -D (don't use O_DIRECT)
	   -tN (timeout in N seconds)

./writeSpeedTest -- writes to N disks using N threads

./writePairTest  -- writes to all combinations of 2 disks in 2 threads, plus analysis


Usage: ./writePairTest -F /dev/sd[cdefghijklmnop]

       ./writeSpeedTest -t10 -D /dev/sdb (non-direct)

       ./readSpeedTest -t10 -D /dev/sdb (non-direct)

       ./readSpeedTest -t20 /dev/sdc (direct test, 20s timeout)
       



 
