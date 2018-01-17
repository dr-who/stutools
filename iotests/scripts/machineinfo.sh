#!/bin/bash

uname -a
free
cat /proc/cpuinfo | grep name
dmidecode | grep -i "clock speed"
dmidecode | grep -i "current speed"
