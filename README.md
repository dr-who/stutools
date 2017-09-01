# stutools

Stu's handy I/O tools:

	./aioRWTest      -- Linux AIO (not POSIX AIO), queue depth, timing, ratio of R/W,
			    sequential, groups of flushing and more

	./readSpeedTest  -- reads from N disks using N threads using fread

	./writeSpeedTest -- writes to N disks using N threads using fwrite/

	./writePairTest  -- writes to all combinations of 2 disks in 2 threads, plus analysis

	./checkDisks     -- checks that we can ok, read and write to each disk

	./watchDisks /dev/sd[cd] -- watch the first N GiB of a disk. -G defaults to 0.01

	./flipBits   /dev/sd[cd] -- randomly change bits


Common options:

	-kN  (N KB, e.g. -k4 is 4096 bytes. Default is -k1024 = 1MB)
	-tN  (timeout in N seconds, defaults to 60)
	-v   (verify writes by checking the checksums)	

Usage:

	./aioRWTest

	./writeSpeedTest /dev/md3

	./writePairTest -F /dev/sd[cdefghijklmnop]

	./writeSpeedTest -t10 -D /dev/sdb (non-direct)

	./readSpeedTest -t10 -D /dev/sdb (non-direct)

	./readSpeedTest -t20 /dev/sdc (direct test, 20s timeout)

	./checkDisks /dev/sd* (check we can open all the disks)

	./checkDisks -m40 /dev/sd* (check we can open all the disks and limit to those over 40MB/s)

	./aioRWTest -f /dev/device

	./aioRWTest -t 100 -S -f /dev/device (read to the same position for 100 seconds)

	./aioRWTest -t 100 -v -S -f /dev/device (location verbosity)

	./aioRWTest -t 100 -p0.5 -v -S -F -f /dev/device (50% R/W mix, flushing after each operation)

	./aioRWTest -t 100 -p0.5 -v -S -S -F -F -f /dev/device
		    (50% R/W mix, changing static location every 10, flushing every 10 operations)

