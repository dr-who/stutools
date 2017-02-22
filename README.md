# stutools

Stu's handy I/O tools

	./readSpeedTest  -- reads from N disks using N threads

	./writeSpeedTest -- writes to N disks using N threads

	./writePairTest  -- writes to all combinations of 2 disks in 2 threads, plus analysis

Options:

	-I   (shortcut to -k4)
	-rN  (random seeks, use at most N GB of the device)
	-kN  (N KB, e.g. -k4 is 4096 bytes. Default is -k1024 = 1MB)
	-D   (don't use O_DIRECT)
	-tN  (timeout in N seconds, defaults to 60)
	-v   (verify writes by checking the checksums)	
	-fN  (perform fdatasync() every N seconds)

Usage:

	./writeSpeedTest /dev/md3

	./writePairTest -F /dev/sd[cdefghijklmnop]

	./writeSpeedTest -t10 -D /dev/sdb (non-direct)

	./readSpeedTest -t10 -D /dev/sdb (non-direct)

	./readSpeedTest -t20 /dev/sdc (direct test, 20s timeout)
 
