#! /bin/bash



# Check if arg given
if [ -z "$1" ]
then
    echo "****"
    echo "  Error: No drive passed in."
    echo ""
    echo "    # ./test.sh <drive>"
    echo "    or"
    echo "    # make test DRIVE=<drive>"
    echo "****"
    exit
fi

echo "Using $1"

# Check for chronic (output suppression)
PREFIX=""
chronic echo "test" 2> /dev/null
if [ $? -eq 0 ]
then
    PREFIX=chronic
else
    echo "To suppress output, '# apt install moreutils'"
fi

# Fail on first error & print cmds
CMD=$(echo "$PREFIX ./aioRWTest -f $1")
set -e
set -x


# Random RW
$CMD -w -v -s0 -t1 -k4-1024
$CMD -r -s0 -t1 -k4-1024
$CMD -wr -v -s0 -t1 -k4-1024
$CMD -s1 -w -P 10000 -k4 -G0.1 -t1

# Single Sequential RW
$CMD -w -v -s1 -t1 -k4-1024
$CMD -r -s1 -t1 -k4-1024
$CMD -wr -v -s1 -t1 -k4-1024

# Multiple Sequential RW
$CMD -w -v -s8 -t1 -k4-1024
$CMD -r -s8 -t1 -k4-1024
$CMD -wr -v -s8 -t1 -k4-1024

# Percentages
$CMD -p0.25 -t1 -v
$CMD -p0.75 -t1 -v

# Flushes
$CMD -t1 -P1 -F 
$CMD -0 -F -t1

# Multiple positions over drive
$CMD -w -v -s128 -P128 -t1 -k4-1024
$CMD -s -j10 -t2

# Multiple Devices
$PREFIX ./aioRWTest -I <(printf "$1\n$1") -r -t0

# Table Mode (Keep last because it takes the longest and it should be fine to exit here)
$CMD -t1 -T
