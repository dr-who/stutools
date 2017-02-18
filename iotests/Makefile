GIT_VERSION := $(shell git describe --abbrev=6 --dirty --always --tags)
CC=gcc
CFLAGS=-Wall --std=c99 -Werror -DVERSION=\"$(GIT_VERSION)\" -O2

all: readSpeedTest writeSpeedTest writePairTest 

readSpeedTest: readSpeedTest.c utils.o logSpeed.o shmem.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm -lrt

writeSpeedTest: writeSpeedTest.c utils.o logSpeed.o shmem.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm -lrt

writePairTest: writePairTest.o utils.o logSpeed.o shmem.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm -lrt

mtWriteSpeedTest: mtWriteSpeedTest.o utils.o logSpeed.o shmem.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm -lrt

mtReadSpeedTest: mtReadSpeedTest.o utils.o logSpeed.o 
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm


utils.o: utils.c utils.h

logSpeed.o: logSpeed.c logSpeed.h

shmem.o: shmem.c shmem.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

clean:
	- rm -f readSpeedTest writeSpeedTest writePairTest logSpeedTest *.o \
	mtReadSpeedTest mtWriteSpeedTest


