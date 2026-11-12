
## Block sizes and protection information (lio-ram)

- 512 and 4096 byte logical blocks: `spit-blk4k-ram` is 4096. LIO takes only these two.
- 64 KiB logical blocks: not presentable by LIO on Linux (the block layer caps logical blocks at the page
  size), so there is no 64 KiB target. Report that as a target limit, not a drive failure.
- PI type 1 (`spit-pi1-ram`) and type 3 (`spit-pi3-ram`). PI type 2 is refused by LIO, so it has no target.
- Exabyte: `spit-exa-ram` (1 EiB, fileio on tmpfs). An LVM thin variant is in progress on VG `spit-thin`.
