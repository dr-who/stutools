# stutools C suite

The original C tools are retained here. Rust spit 2.0 lives in `../rust/`.

Stu's Powerful I/O Tester (spit)

      ./build/spit/spit   - powerful test program, sequential random, queue depth, timings and more.

      Tools for generating benchmarking, with an aim to be able to
      regenerate the important tests from the SNIA (Storage Networking
      Industry Association) suite. And a lot more!

## Build and install the C suite

From this `c/` directory on Debian/Mint/RHEL/Rocky/Alma/Fedora:

```sh
./install-deps.sh
make JOBS=4
make test
sudo make install
```

CMake 3.20+ and the listed distro development packages are required. The helper
installs them with apt or dnf. Linux defaults to `/usr`: the C tester installs as
**`/usr/bin/spitc`**, alongside fsfiller and the other C utilities. The build-tree
binary retains its target path `build/spit/spit`. Override `PREFIX` deliberately
if a different destination is needed.

For native packages use `make dist`, `make deb`, or `make rpm`. Debian recipes
are in `debian/`; the RPM spec is `packaging/stutools-c.spec`. The package name
is `stutools-c`, compatible with Rust `stutools` 2.0. Offline source archives
include version metadata and are built with `make source`.

Direct CMake usage must be out of source:

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --parallel 4
ctest --test-dir build --output-on-failure -L smoke
sudo cmake --install build
```

On macOS/Windows CMake builds the limited portable file backend; the full
original Linux utilities require Linux. Use the Rust release for native spit.

## Existing files and error handling

Block-device ranges must fit entirely within the device. Invalid ranges are
rejected before I/O; they are never reset to the start of the device.

For an existing regular file, `spit` uses its current size when `-G` is omitted.
A smaller `-G` limits the tested range without truncating the file. A range beyond
EOF is rejected unless a write job explicitly supplies `--recreate`; that option
allows the file to be recreated when its size differs. Read-only jobs require an
existing file and never recreate it. These rules also apply to `-F` prefix files.
Write jobs still overwrite the positions selected by their workload.

Short reads (including EOF), short writes, I/O errors, and a timeout while draining
outstanding requests fail the run. Only complete transfers count as successful
I/Os, and byte counters use the actual transferred length. Cleanup keeps buffers
alive until the kernel AIO context is destroyed; this can still wait on stuck I/O.

Run the focused regression tests after building:

      python3 tests/test_spit_safety.py build/spit/spit

To run the optional block-device regression tests with loop-device permissions:

      python3 tests/test_spit_block.py build/spit/spit

This creates and detaches its own temporary loop device.

The build refreshes the Git version at build time, marking modified sources with
`-dirty`. The generated header lives in the build directory.

The tests use temporary files and a Linux/glibc fault-injection library compiled
with `cc`. No real block device is used.

## As a docker container

      git clone https://github.com/dr-who/stutools.git
      docker build -t spitc stutools/c/
      docker run -it spitc
      docker run -it --device /dev/sdk:/dev/sdk spit
      # spit -f /dev/sdk -rs0

## Man page

      man -l spit/spit.1

## Write tests

      spitc -f device -c wk64        # test seq writes 64KiB, starting a random location

      spitc -f device -c wk64z       # test seq writes 64KiB, starting from block 0.

      spitc -f device -c wk64s0      # random writes 64KiB

      spitc -f device -c wk64s0j64   # random writes 64KiB, using 64 threads with overlapping ranges

      spitc -f device -c wk64s0j64G_ # random writes 64KiB, using 64 threads, non overlapping ranges

      spitc -f device -c wk64s0j64G10-20 # random writes 64KiB, 64t, overlapping LBA range [10-20] GB

      spitc -f device -c wk64s0j64G10_20 # random writes 64KiB, 64t, non-overlapping LBA range [10-20] GB
                                     note the underscore range delimits non-overlapping ranges.

      spitc -f device -c wk64s0j32 -v # add -v to verify the writes

      spitc -f /dev/md0 -c ... -O rawdevices.txt  # perform actions on a meta device and show amplification

## Zoned devices

     blkzone reset -o 39015415808 -c 4096 /dev/sdg        #reset the zone for writing. Offset in 512 byte sectors.

     spitc -f /dev/sdg -c zws1k64x1 -G 39015415808s+1GiB  # spit s suffix is 512 byte sectors

## Dump out the commands

      spitc -f .... -c ... -d 100     # dumps out the first 100 commands, to see what's happening

## IOPS latency

      spitc -f device -c wk64s0S1000q1 -P pos.txt   # random write, 64K, target 1,000 IOPS with qd=1. Display latency

               # all timing information is in the file specified with -P

## Write/read alternating

      spitc -f device -c 'w s1 k1024 z m1'        # write then read back the same block, alternating 1:1

      spitc -f device -c 'w s1 k1024 z m100'      # write 1MiB blocks sequentially, every 100 writes read the last 100


## TRIM/Discard tests

      spitc -f device -c ts0 -G10    # perform random trims

      spitc -f device -c wx5 -G10 -T # write 10GiB at a time, trimming the LBA before each iteration ('T'), repeat 5 times.

## Read and more complex tests

      spitc -f device -c rs0         # random reads 4KiB

      spitc -f device -c rs0j64      # random reads 4KiB, 64 threads

      spitc -f device -c rs0j64G_    # random reads 4KiB, 64 threads, non-overlapping LBA range

      spitc -f device -c rs0j2G_ -d 10 # dump the first 10 positions in each thread

      spitc -f device -c wk64j3 -c rk64j2    # three writers, two reader threads

      spitc -f device -c wk4-128j3 -P positions.txt  # variable block range, three overlapping
                                    range, positions written out

      spitchecker positions.txt     # verify the previous writes, say after a reboot.
                                    Same code as the -v verification option.

      spitc -f device -c wk64s0G10-20j4 -c rk4-8G20-100j64   # two ranges [10-20]
                                    and [20-100] GB, varying threads and I/O type.

      spitc -f device -c wk4P1000000 # write to only 1,000,000 positions for cache testing

      spitc -f device -c wk2048x1    # x1 means write the LBA size once (or a Glow-high range),
                                    instead of being time limited.

## Multiple input files and utilization

      spitc -I listofdevices.txt -c w  # will read a list of devices from the file.

      spitc -f device -c ... -O listofdevices.txt  # will display the r/w/IO operations on the devices.
                                    This is used to see meta-device amplification and phy IO.

## Limits

      The limits can be per LBA multiple using the 'x' option. e.g. x1 is the LBA size.

      Or by position using the 'X' option. X1 is the number of positions.

      Or by time using the local 'T' (time) option. T2 is two seconds. The xXT options can't be combined.

## NUMA binding

      By default the spit threads are distributed evenly between the NUMA nodes. To disable
      NUMA pinning use the -u option. For example with j51 (51 threads) spit may output:

      *info* NUMA[0] 26 pinned on 24 hardware threads, NUMA[1] 25 pinned on 24 hardware threads,

      To bind all threads to NUMA node 0 use -U 0. The output will then be:

      *info* NUMA[0] 51 pinned on 24 hardware threads, NUMA[1] 0 pinned on 24 hardware threads,

      The -U format allows a list (-U 1,2) and a range (-U 0-3,5).

## File system tests

      The -f (lowercase) filename will create a SINGLE file with that name. Capital -F prefix will
      create MULTIPLE files, one per thread.

      spitc -f filename -G 10 -c wk4-8s0  # create a 10 GiB file and perform random variable length
                                   writes on the file.

      spitc -F /mnt/prefix -G 10 -c wk64j128  # In 128 threads, create a file per thread, 10 GB in size,
                                   run for the default 10 seconds.

      spitc -F /mnt/prefix -G 10 -c wk64j128x1  # In 128 threads, create a file per thread, 10 GB in size,
                                   write each file once. e.g. create 128 x 10 G files.


## SNIA Common tests
      spitc -f device -p G -c wk64    # precondition random 4k writes, test seq writes 64KiB

      spitc -f device -p G -c rs0     # precondition random 4k writes, test rand reads 4KiB

      spitc -f device -p G -c mP100000 # precondition, then R/W mix on 100,000 positions

## Usage

      Usage:
       spit [-f device] [-c string] [-c string] ... [-c string]

      Create positions:
       spitc -f dev -c rs0            # defaults to use RAM and generate unique positions. MIN(15GB,freeRAM/2)
       spitc -f dev -c rs1P100z       # the first 100 positions in a device, starting from zero
       spitc -f dev -c rs1P100        # the first 100 positions in a device, offset by a random amount
       spitc -f dev -c rs0P100        # the first 100 positions in a device, randomised
       spitc -f dev -c rs1P+100z      # 100 positions equally spaced across the array, starting from zero
       spitc -f dev -c rs1P+100       # 100 positions equally spaced across the array, offset by a random amount
       spitc -f dev -c rs0P+100       # 100 positions equally spaced, randomised
       spitc -f dev -c rs1P-100       # subsample with replacement, 100 positions randomly distributed over the array
       spitc -f dev -c rs0P-100       # effectively the same as above with s1. But re-randomised.
       spitc -f dev -c rs1P.100z      # 100 equally spaced, alternating start,end,s+1,e-1... centre
       spitc -f dev -c rs1P.100       # effectively the same as above with s1 but with random offset
       spitc -f dev -c rs0 -L20       # Use 20GiB of RAM to generate more unique positions
       spitc -f dev -c rs0j2G_        # Make two threads, but first split the LBA range into two.

      Copying regions:
       spitc -f dev -c c              # will copy from a region to a destination 1/2 the LBA away, 4 KiB blocks
       spitc -f dev -c ck64           # will copy using 64 KiB blocks
       spitc -f dev -c ck64j4G_       # Divides the space into 4 regions, runs 4 threads performing local copies per region
       spitc -f dev -c ckz64-2048      # Will perform 2 MiB worth of 64 KiB reads, then a single 2 MiB write, starting from 0

      Varying executions: (time, position and LBA controls)
       spitc -f dev -c rs0            # run for 10 seconds by default
       spitc -f dev -c rs0 -t10       # run for 10 seconds
       spitc -f dev -c rs0 -t-1       # run for ever
       spitc -f dev -c rs0x1          # cover the entire LBA one time
       spitc -f dev -c rs0x2          # cover the entire LBA two times
       spitc -f dev -c rs0P100x1      # perform a LBA worth of those first 100 positions
       spitc -f dev -c rs0P100x3      # perform 3xLBA worth of those first 100 positions
       spitc -f dev -c rs0P2000X1     # execute the 2,000 IO operations
       spitc -f dev -c rs0P2000X2     # execute the 2,000 IO operations, twice
       spit ..-t 60 -c ws1zM2P1000=  # = alternates r/w per pass. Write 1000, then read 1000, continue for 60s

      Sequential/random/striping:
       spitc -f dev -c s1z            # sequential, one linear region, starting from zero
       spitc -f dev -c s1             # sequential, one linear region, randomly offset
       spitc -f dev -c s0             # Not sequential, i.e. random
       spitc -f dev -c s2             # Two sequential regions. Divided the thread/region into two
       spitc -f dev -c s9             # Nine sequential regions. Divided the thread/region into nine...
       spitc -f dev -c s-1z           # Reverse sequential starting from the end of the array
       spitc -f dev -c s-1            # Reverse sequential, randomly offset
       spitc -f dev -c s-2            # Two sequential regions, reversed, randomly offset

      Between linear and random:
       spit ... s-1                  # monotonically decreasing
       spit ... s0                   # random
       spit ... s0.05                # Almost entirely random with a few monotonically increasing positions
       spit ... s0.95-20             # With a probability of 0.05 swap with a position +/- 20 positions
       spit ... s0.98                # With a probability of 0.02 swap with any other position
       spit ... s1                   # monotonically increasing

      Positions/latencies:
       spitc -P filename              # dump positions to filename
                                     # file contains device, position, size, timing, latency per sample, median latency
       spitc -P -                     # dump positions to (stdout) and stream raw IOs without collapsing
                                     # column 2 (byte offset), 6 (operation), 7 (size)
                                     # column 12/13 (start/fin time), column 14 (mean latency)
                                     # column 15 (#samples), 16 (median latency)
        ... -S positions             # read all the positions from 3 columns. position action and size

      LBA coverage: (using lowercase x)
       spitc -c P10x1                 # write 10 positions until the entire device size is written
       spitc -c P10x3                 # write 10 positions until the entire device size is written three times
       spitc -c x5                    # writing the block device size 5 times, not time based

      Position coverage: (using capital X)
       spitc -f ... -c P10X100        # multiply the number of positions by X, here it's 100, so 1,000 positions
       spitc -f .... -c wns0X10       # writing the number of positions 10 times, not time based
       spitc -c P10X1                 # write 10 positions
       spitc -c P10000X100            # write the same 10,000 positions 100 times

      Complete coverage: (positions required, size of `LBAbytes / (RAMbytes / 64)`)
       spitc -f ... -c ..C            # the 'C' command will check the complete LBA is covered, or exit(1)

      Displays: (-s 1 default, once per second)
       spitc -s 0.1 -i 5              # display every 0.1 seconds, ignore first 5 GB

      Queue depth: (IO in flight)
       spitc -f .... -c q128          # per job max queue depth (max inflight is 128, submit one at a time)
       spitc -f .... -c q32-128       # set the min queue depth to 32, submit at most 32 IOs at a time, max 128
       spitc -f .... -c q128-128      # set queue depth to fixed at 128 IOs in flight at all time. Uses less CPU.

      Speed targets: (IOPS target)
       spit ... -c ws1S1000q1        # Target 1000 IOPS, with QD=1
       spitc -c ws1zk10001J2S1000     # Writing monotonically, but from alternating between two threads at 1 MB x 1000/s = 1 GB/s
       spitc -F. -c ws1zx1j64S100q1 -G1 # creates files from .0001 to .0128, with IOPS targets
       spit ... -c ws1S100           # Targets slower IOPS, S100 targets 100 IOPS per thread, with default qd

      Writing/pausing:
       spitc -f device -c W5          # do 5 seconds worth of IO then wait for 5 seconds
       spitc -f device -c W0.1:4      # do 0.1 seconds worth of IO then wait for 4 seconds
       spitc -f ... -c w -cW4rs0      # one thread seq write, one thread, run 4, wait 4 repeat
       spitc -f ... -c ws1W2:1 -t60   # Alternate run for 2 seconds, wait for 1 second
       spitc -f .. -c ws0S1           # Write a 4KiB block, randomly, 1 IO per second. The *slow* Loris

      NUMA control:
       spitc -f -c ..j32 -u           # j32, but do not pin the threads to specific NUMA nodes
       spitc -f -c ..j32 -U 0         # j32, pin all threads to  NUMA node 0
       spitc -f -c ..j32 -U 0,1       # j32, split threads evenly between NUMA node 0 and 1
       spitc -f -c ..j32 -U 0-2       # j32, split threads evenly between NUMA node 0, 1 and 2
       spitc -f -c ..j32 -U 0,0,1     # j32, allocate threads using a 2:1 ratio between NUMA node 0 and 1

      See/dump the actions/positions:
       spitc -f ... -c ... -d 10      # dump the first 10 positions/actions

      LBA ranges/partitioning:
       spitc -f device -c r -G 1      # 0..1 GB device size
       spitc -f device -c r -G 384MiB # -G without a range is GB. {M,G,T}[i*]B
       spitc -f device -c r -G 100GB  # Support GB/GiB etc  spit -f device -c r -G 1-2    # Only perform actions in the 1-2 GB range
       spitc -f device -c r -E -G 2-5 # if the -E argument is *before* -G, G values are percentages. e.g. 2%-5%
       spitc -f device -c r -G 2-5 -E # NOTE: This doesn't work. -E must be before -G
       spitc -c wx3 -G4 -T            # perform pre-DISCARD/TRIM operations before each round
       spitc -c wx1G0-64k4zs1         # Write from [0,64) GiB, in 4KiB steps, sequentially
       spitc -c wx1G0-64k4zs1K20      # Write from [0,64) GiB, in 4KiB steps, writing 1 in 20.
       spitc -c wG_j4                 # The _ represents to divide the G value evenly between threads
       spitc -c ws1G1-2 -c rs0G2-3    # Seq w in the 1-2 GB region, rand r in the 2-3 GB region
       spitc -f ... -c P10G1-2        # The first 10 positions starting from 1GiB. It needs the lower range.
       spitc -c ws1G5_10j16           # specify a low and high GiB range, to be evenly split by 16 threads (_)

      Logging for experiments:
       spitc -f ... -l logfile        # will append the run results to 'logfile'. Works well with ./combo expansion
       spitc -B bench -M ... -N ...   # See the man page for benchmarking tips

      Block sizes:
       spitc -f device -c k8          # set block size to 8 KiB
       spitc -f device -c k4-128      # set block range to 4 to 128 KiB, every 4 KiB
       spitc -f device -c k4:1024     # set block range to 4 to 1024 KiB, in powers of 2
       spitc -f ... -c wM1            # set block size 1M

      Range resets:
       spitc -f device -c s1k8A1        # set block size to 8 KiB, randomize next pos after 1 MiB of data
       spitc -f device -c s1k4-16A0.5-8 # set block size to 4-16 KiB, randomize after 0.5 to 8 MiB
       spitc -f device -c s1k4:16A0.5:8 # As above but power of 2 ranges

      I/O amplification: (-O underlying_devices.txt)
       spitc -f dev -O devices.txt    # specify the raw devices for amplification statistics
       spitc -f dev -O <(echo dev)    # use BASH syntax for creation of a virtual inline fd
       spit .. -c rs1k4q1 -O ..      # random 4 KiB, times each block read when contiguous
       spit .. -c zrs1k4q1j128 -O .. # read the same ranges from 128 threads, are they cached
       spit .. -c rs0k4q1 -O ..      # random 4 KiB, how much extra is read
       spit .. -c rs1M2q1 -O ..      # sequential random 2 MiB, check amp

      Read-ahead (testing seq reads with queue depth of 1/q1)
       spit .. -c rs1k4q1 -O ..      # sequential 4 KiB reads, check amplification
       spit .. -c rs1k4-128q1 -O ..  # sequential 4-128 KiB reads, check amplification
       spit .. -c rs1k64A4q1 -O ..   # 64 KiB reads, reset position every 4 MiB
       spit .. -c rs1k64K4q1 -O ..   # 64 KiB reads, only doing every 4th action
       spit .. -c rzs1k1024A8q1      # start at zero, seq, 1M blocks, 8M of contig then new pos
       spit .. -c rs1M2q1 -O ..      # seq 2 MiB reads
       spit .. -c rs-1M2q1 -O ..     # reverse seq/backwards 2 MiB reads

      Complex Read-ahead
       spit .. -c rs1k4-64A8 -c rs0j8 # Testing one thread with RA, multiple random other threads
       spit .. -c rs1k4A0.25-4       # the A value will be a random value between 256 KiB and 4 MiB
       spit .. -c rs1k4A1:8          # A will be a power-of-two random value in 1-8 MiB. 1,2,4 or 8 MiB

      I/O write verification: (-v to verify)
       spitc -v                       # verify the writes after a run
       spit ... -c ws0u -v           # Uses a unique seed (u) per operation (mod 65536)
       spit ... -c ws0U -v           # Generates a read immediately after a write (U), tests with qd=1
       spit ... -c ws0UG_j32 -v      # Generates r/w pairs with unique seeds, as above, unique thread ranges

      Preconditioning:
       spitc -p G100s1k64             # precondition job, sequential, 64 KiB blocks
       spitc -p G -p Gs1              # precondition job, writing random, 100% LBA, then seq job
       spitc -p G100                  # precondition job, writing random overwrite LBA size
       spitc -p f5 -f device -c ...   # Precondition/max-fragmentation with 5% GC overhead, becomes K20.
                                     # the -p f5 commands iterates across the LBA in 64G chunks as previous two commands.

      O_DIRECT vs pagecache: (default O_DIRECT)
       spitc -f ... -c rD0            # 'D' turns off O_DIRECT

      Misc examples:
       spitc -f device -c ... -c ... -c ... # defaults to 10 seconds
       spitc -f device -c r           # seq read (defaults to s1 and k4)
       spitc -f device -c w           # seq write (s1)
       spitc -f device -c w -L1       # Limit RAM use to 1 GiB. 48 bytes is used per position
       spitc -f device -c w           # Default RAM allocated to positions is MIN(15GB, free RAM/2)
       spitc -f device -c rs0         # random, (s)equential is 1
       spitc -f device -c rs1         # sequential
       spitc -f device -c rzs1        # sequential starting from position 0
       spitc -f device -c rzs-1       # reverse sequential starting from end of the device
       spitc -f device -c rs0-size    # random, (s)equential is 0, max contig is size KiB
       spitc -f device -c ws2         # 2 contiguous regions on the device. Add -d 10 to dump positions
       spitc -f device -c ws128       # 128 contiguous regions on the device
       spitc -f device -c "r s128 k4" -c 'w s4 -k128' -c rw
       spitc -f device -b 10240000    # specify the max device size in bytes
       spitc -f ... -t 50             # run for 50 seconds (-t -1 is forever)
       spitc -f -c ..j32              # duplicate all the commands 32 times. Pin threads to each NUMA node.
       spitc -f ... -c wR42           # set the per command seed with R
       spitc -f ... -c wF             # (F)lush after every write of FF for 10, FFF for 100 ...
       spitc -f ... -c rrrrw          # do 4 reads for every write
       spitc -f ... -c rw             # mix 50/50 reads/writes
       spitc -f ... -c n              # shuffles the positions every pass
       spitc -f ... -c N              # adds an offset to the positions every pass
       spitc -f ... -t -1             # -t -1 is run forever
       spitc -f ... -c p0.9           # set the r/w ratio to 0.9
       spitc -f ... -c wz             # sequentially (w)rite from block (z)ero (instead of random position)
       spitc -f ... -c m              # non-unique positions, read/write/flush like (m)eta-data
       spitc -f ... -c mP4000         # non-unique 4000 positions, read/write/flush like (m)eta-data
       spitc -f ... -c s1n            # do a sequential pass, then shuffles the positions
       spitc -f ... -c rL4            # (L)imit positions so the sum of the length is 4 GiB
       spitc -f ... -c O              # One-shot, not time based
       spitc -f ... -c t2             # specify the time per thread
       spitc -f ... -c ws1F4          # jumble/reverse groups of 4 positions
       spitc -f ... -c wx20           # sequentially (s1) write 20x LBA, no time limit
       spitc -f ... -c ws0            # random defaults to 3x LBA
       spitc -I devices.txt -c r      # -I is read devices from a file
       spitc -f .... -R seed          # set the initial seed, j will increment per job
       spitc -c P10000                # write the same 10,000 positions based on the time
       spitc -c P-10000               # A -ve P number, is determine 10,000 positions with replacement. Don't verify
       spitc -c wZ1                   # Z is the starting offset. -z is -Z0
       spitc -F fileprefix -c ..j128  # creates files from .0001 to .0128
       spitc -e "5,echo five"         # exec a bash -c CMD string after 5 seconds, quotes are required
       spitc -c wk1024za7             # every 'a' MiB of operations perform a jump back to the start of device. Dump with -d to see
       spitc -c ts0                   # Use a sync DISCARD/TRIM I/O type
       spitc -c rrwts0                # 50% read, 25% writes and 25% trim I/O random operations
       spit ... -c rs0Y1000          # Limit the number of positions to process to 1,000 then end the run
       spitc -c ... -D                # Display the 'D'ate/time per line
       spitc -f .. -c ws0J8           # Use 8 threads, but only do every 8th IO in each thread. For NUMA/S testing

## Code Formatting

       apt install astyle
       ./utils/code-format

## Errors in block device

To create a 256 MiB block device:

 root# modprobe brd rd_nr=1 rd_size=$[256*1024]
 root# dmsetup create errdev0
 0 261144 linear /dev/ram0 0
 261144 5 error
 261149 263139 linear /dev/ram0 261149
 ^D

 # lsblk -b /dev/mapper/errdev0
 NAME    MAJ:MIN RM      SIZE RO TYPE MOUNTPOINTS
 errdev0 253:0    0 268435456  0 dm

 root# ls -l /dev/mapper/errdev0
 lrwxrwxrwx 1 root root 7 Jun 27 14:03 /dev/mapper/errdev0 -> ../dm-6

The device mapper ranges are in 512 byte sectors. So the error range
will start at 512 x 261144 = 133705728.

To test the device:

 spit -f /dev/mapper/errdev0 -c wzs1 -P positions.txt

This displays a error at position 133705728 and doesn't output the
positions.txt file due to error and the positions.txt file will be 0
bytes. To ignore the I/O errors:

 spit -f /dev/mapper/errdev0 -c wzs1 -P positions.txt -E

The -E option continues and writes the positions.txt file. spitchecker can now verify the positions.

 spitchecker positions.txt

To attempt to see the block contents, you'd try this but it'll fail with I/O error.

 devcontents -f /dev/mapper/errdev0 -b 133705728

To remove the devices:

 root# dmsetup remove errdev0
 root# rmmod brd

### Dropping a drive

 echo 1 | sudo tee /sys/block/sd{drive}/device/delete

and to rescan...

 echo "- - -" |tee /sys/class/scsi_host/*/scan
