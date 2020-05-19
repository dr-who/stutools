# stutools

Stu's Powerful I/O Tester (spit)

      ./spit/spit   - powerful test program, sequential random, queue depth, timings and more.

      Tools for generating SNIA (Storage Networking Industry Association) compatible benchmarking.

## Dependencies

      apt install libaio-dev libnuma-dev

## Install

      cmake .
      make -j

## Man page

      man -l spit/man.1

## Write tests

      spit -f device -c wk64        # test seq writes 64KiB, starting a random location

      spit -f device -c wk64z       # test seq writes 64KiB, starting from block 0.

      spit -f device -c wk64s0      # random writes 64KiB

      spit -f device -c wk64s0j64   # random writes 64KiB, using 64 threads with overlapping ranges

      spit -f device -c wk64s0j64G_ # random writes 64KiB, using 64 threads, non overlapping ranges

      spit -f device -c wk64s0j64G10-20 # random writes 64KiB, 64t, overlapping LBA range [10-20] GB

      spit -f device -c wk64s0j64G10_20 # random writes 64KiB, 64t, non-overlapping LBA range [10-20] GB
                                     note the underscore range delimits non-overlapping ranges.

      spit -f device -c wk64s0j32 -v # add -v to verify the writes

      spit -f /dev/md0 -c ... -O rawdevices.txt  # perform actions on a meta device and show amplification

## Read and more complex tests

      spit -f device -c rs0         # random reads 4KiB

      spit -f device -c rs0j64      # random reads 4KiB, 64 threads

      spit -f device -c rs0j64G_    # random reads 4KiB, 64 threads, non-overlapping LBA range

      spit -f device -c rs0jj2G_ -d 10 # dump the first 10 positions in each thread

      spit -f device -c wk64j3 -c rk64j2    # three writers, two reader threads

      spit -f device -c wk4-128j3 -P positions.txt  # variable block range, three overlapping
                                    range, positions written out

      spitchecker positions.txt     # verify the previous writes, say after a reboot.
                                    Same code as the -v verification option.

      spit -f device -c wk64s0G10-20j4 -c rk4-8G20-100j64   # two ranges [10-20]
                                    and [20-100] GB, varying threads and I/O type.

      spit -f device -c wk4P1000000 # write to only 1,000,000 positions for cache testing

      spit -f device -c wk2048x1    # x1 means write the LBA size once (or a Glow-high range),
                                    instead of being time limited.

## Multiple input files and utilization

      spit -I listofdevices.txt -c w  # will read a list of devices from the file.

      spit -f device -c ... -O listofdevices.txt  # will display the r/w/IO operations on the devices.
                                    This is used to see meta-device amplification and phy IO.

## NUMA binding

      By default the spit threads are distributed evenly between the NUMA nodes. To disable
      NUMA pinning use the -n option. For example with j51 (51 threads) spit may output:

      *info* NUMA[0] 26 pinned on 24 hardware threads, NUMA[1] 25 pinned on 24 hardware threads,

      To bind all threads to NUMA node 0 use -U 0. The output will then be:

      *info* NUMA[0] 51 pinned on 24 hardware threads, NUMA[1] 0 pinned on 24 hardware threads,

## File system tests

      The -f (lowercase) filename will create a SINGLE file with that name. Capital -F prefix will
      create MULTIPLE files, one per thread.

      spit -f filename -G 10 -c wk4-8s0  # create a 10 GiB file and perform random variable length
                                   writes on the file.        

      spit -F /mnt/prefix -G 10 -c wk64j128  # In 128 threads, create a file per thread, 10 GB in size,
      	                           run for the default 10 seconds.

      spit -F /mnt/prefix -G 10 -c wk64j128x1  # In 128 threads, create a file per thread, 10 GB in size,
                                   write each file once. e.g. create 128 x 10 G files.


## SNIA Common tests
      spit -f device -p G -c wk64    # precondition random 4k writes, test seq writes 64KiB

      spit -f device -p G -c rs0     # precondition random 4k writes, test rand reads 4KiB

      spit -f device -p G -c mP100000 # precondition, then R/W mix on 100,000 positions

## Usage

      cd spit
      ./spit

      Usage:
      
      spit -f device -c ... -c ... -c ... # defaults to 10 seconds
      spit -f device -c r           # seq read (s1)
      spit -f device -c w           # seq write (s1)
      spit -f device -c rs0         # random, (s)equential is 0
      spit -f device -c ws128       # 128 parallel (s)equential writes
      spit -f device -c rs128P1000  # 128 parallel writes, 1000 positions
      spit -f device -c k8          # set block size to 8 KiB
      spit -f device -c k4-128      # set block range to 4 to 128 KiB
      spit -f device -c W5          # wait for 5 seconds before commencing I/O
      spit -f device -c "r s128 k4" -c 'w s4 -k128' -c rw
      spit -f device -c r -G 1      # 1 GiB device size
      spit -f ... -t 50             # run for 50 seconds (-t -1 is forever)
      spit -f ... -j 32             # duplicate all the commands 32 times
      spit -f ... -f ...-d 10       # dump the first 10 positions per command
      spit -f ... -c rD0            # 'D' turns off O_DIRECT
      spit -f ... -c w -cW4rs0      # one thread seq write, one thread wait 4 then random read
      spit -f ... -c wR42           # set the per command seed with R
      spit -f ... -c wF             # (F)lush after every write of FF for 10, FFF for 100 ...
      spit -f ... -c rrrrw          # do 4 reads for every write
      spit -f ... -c rw             # mix 50/50 reads/writes
      spit -f ... -c rn -t0         # generate (n)on-unique positions positions with collisions
      spit -f ... -t -1             # -t -1 is run forever
      spit -f ... -c wz             # sequentially (w)rite from block (z)ero (instead of random position)
      spit -f ... -c m              # non-unique positions, read/write/flush like (m)eta-data
      spit -f ... -c mP4000         # non-unique 4000 positions, read/write/flush like (m)eta-data
      spit -f ... -c n              # rerandomize after every set of positions
      spit -f ... -c s1n            # do a sequential pass, then randomise the positions
      spit -f ... -c rL4            # (L)imit positions so the sum of the length is 4 GiB
      spit -f ... -c P10x100        # multiply the number of positions by x, here it's 100
      spit -f ... -c wM1            # set block size 1M
      spit -f ... -c O              # One-shot, not time based
      spit -f ... -c t2             # specify the time per thread
      spit -f ... -c wx2O           # sequentially (s1) write 200% LBA, no time limit
      spit -f ... -c ws0            # random defaults to 3x LBA
      spit -f ... -c ws1W2T2 -t60   # Alternate wait 2, run for 2 seconds
      spit -I devices.txt -c r      # -I is read devices from a file
      spit -f .... -R seed          # set the initial seed, -j will increment per job
      spit -f .... -Q qd            # set the per job default queue depth
      spit -f .... -c wns0X10       # writing 10 times, not time based
      spit -f -p G -p Gs1           # precondition job, writing random, 100% LBA, then seq job (handy for SNIA type)
      spit -f -p G100               # precondition job, writing random overwrite LBA size
      spit -p G100s1k64             # precondition job, sequential, 64 KiB blocks
      spit -f meta -O devices.txt   # specify the raw devices for amplification statistics
      spit -s 0.1 -i 5              # and ignore first 5 seconds of performance
      spit -v                       # verify the writes after a run
      spit -P filename              # dump positions to filename
      spit -c wG_j4                 # The _ represents to divide the G value evenly between threads
      spit -B bench -M ... -N ...   # See the man page for benchmarking tips
      spit -F fileprefix -j128      # creates files from .0001 to .0128
      spit ... -c ws0u -v           # Uses a unique seed (u) per operation (mod 65536)
      spit ... -c ws0U -v           # Generates a read immediately after a write (U), tests with QD=1
      spit ... -c ws0UG_ -v -j32    # Generates r/w pairs with unique seeds, as above, unique thread ranges
      spit ... -c ws1S250           # S option targets a speed in MB/s by adding usleep() between operations. 
