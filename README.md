# stutools

Stu's Powerful I/O Tester (spit)

      ./spit/spit   - powerful test program, sequential random, queue depth, timings and more.

## Dependencies

      apt install libaio-dev

## Install

      cmake .
      make -j

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
