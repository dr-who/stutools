#!/bin/bash

uname -a
free
cat /proc/cpuinfo | grep name
dmidecode | grep -i "clock speed"
dmidecode | grep -i "current speed"

mkdir -p /mnt/stu
mount -t tmpfs -o size=1024M,mode=0755 tmpfs /mnt/stu
dd if=/dev/zero of=/mnt/stu/wow >/dev/null 2>&1
losetup /dev/loop7 /mnt/stu/wow
../aioRWTest -w -f /dev/loop7 -t2 2>&1 | grep -i total
../aioRWTest -r -f /dev/loop7 -t2 2>&1 | grep -i total
losetup -d /dev/loop7
/bin/rm -f /mnt/stu/wow
umount /mnt/stu
