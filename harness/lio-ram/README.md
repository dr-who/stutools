
## Block sizes and protection information (lio-ram)

- 512 and 4096 byte logical blocks: `spit-blk4k-ram` is 4096. LIO takes only these two.
- 64 KiB logical blocks: not presentable by LIO on Linux (the block layer caps logical blocks at the page
  size), so there is no 64 KiB target. Report that as a target limit, not a drive failure.
- PI type 1 (`spit-pi1-ram`) and type 3 (`spit-pi3-ram`). PI type 2 is refused by LIO, so it has no target.
- Exabyte: `spit-exa-ram` (1 EiB, fileio on tmpfs). An LVM thin variant is in progress on VG `spit-thin`.

## Known issue: the standby TPG of spit-alua-ram (open)

- INQUIRY on 127.0.0.8 reports TPGS 0 (no ALUA), while REPORT TARGET PORT GROUPS on the same portal
  answers GOOD and lists no group. The two answers contradict each other (SPC-3 6.4.2 and 6.25).
- `targetcli` shows the group `standby` as Standby, and tpg2's LUN as `(standby)`, so the configuration
  is not yet presented the way the lab's standby LUN (spitlunsbc-2, `standby (group 1)`) is.
- Until this is resolved, the standby cases on 127.0.0.8 are not meaningful: spit cannot tell from TPGS
  that the path is standby, so it judges the refusals a standby port must give as failures. That is a spit
  gap as well: the standby command set must be applied when the ALUA state is standby.
