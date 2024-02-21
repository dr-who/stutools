#!/bin/bash

modprobe brd rd_nr=128 rd_size=104857600

/bin/rm -f /etc/exports

for f in {0..127}
do

    umount /mnt/disk$f
    
    mkfs.xfs /dev/ram$f
    mkdir -p /mnt/disk$f
    chmod 777 /mnt/disk$f
    mount /dev/ram$f /mnt/disk$f
    echo "/mnt/disk$f *(rw,sync)" >>/etc/exports

done

exportfs -a
