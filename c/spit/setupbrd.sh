#!/bin/bash

umount -f /mnt/disk*
rmmod brd
modprobe brd rd_nr=64 rd_size=1024000

/bin/rm -f /etc/exports

for f in {0..63}
do

    umount /mnt/disk$f
    
    mkfs.xfs -f /dev/ram$f
    mkdir -p /mnt/disk$f
    chmod 777 /mnt/disk$f
    mount /dev/ram$f /mnt/disk$f
    echo "/mnt/disk$f *(rw,sync)" >>/etc/exports

done

exportfs -a
