#!/bin/bash

modprobe brd nr_rd=32 rd_size=104857600

/bin/rm -f /etc/exports

for f in 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
do

    umount /mnt/disk$f
    
    mkfs.xfs -f /dev/ram$f
    mkdir -p /mnt/disk$f
    mount /dev/ram$f /mnt/disk$f
    echo "/mnt/disk$f *(rw,sync)" >>/etc/exports

done

exportfs -a
