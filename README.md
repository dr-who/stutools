# stutools
Stu's handy tools

./readSpeedTest  -- reads from N disks using N threads

./writeSpeedTest -- writes to N disks using N threads

./writePairTest  -- writes to all combinations of 2 disks in 2 threads, plus analysis

Common options: -d turns on direct mode (may already be default), -D turns off, -F super fast, -f fast, -S slow 

Usage: ./writePairTest -F /dev/sd[cdefghijklmnop]

 
