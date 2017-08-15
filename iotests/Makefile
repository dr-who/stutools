GIT_VERSION := $(shell git describe --abbrev=6 --dirty --always --tags)
CC=gcc
CFLAGS=-Wall --std=c99 -Werror -DVERSION=\"$(GIT_VERSION)\" -g -O2

all: readSpeedTest writeSpeedTest writePairTest checkDisks flushTestWriter flushTestReader \
	aioRWTest

readSpeedTest: readSpeedTest.c utils.o logSpeed.o 
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm

writeSpeedTest: writeSpeedTest.c utils.o logSpeed.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm

writePairTest: writePairTest.o utils.o logSpeed.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm

mtWriteSpeedTest: mtWriteSpeedTest.o utils.o logSpeed.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm

mtReadSpeedTest: mtReadSpeedTest.o utils.o logSpeed.o 
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm

checkDisks: checkDisks.o utils.o logSpeed.o aioOperations.o
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm -laio

flushTestWriter: flushTestWriter.o utils.o logSpeed.o 
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm 

flushTestReader: flushTestReader.o utils.o logSpeed.o 
	gcc -Wall --std=c99 $^ -o $@ -lpthread -lm

aioRWTest: aioRWTest.o aioRequests.o logSpeed.o utils.o diskStats.o
	gcc -Wall --std=c99 $^ -o $@ -lm -laio


utils.o: utils.c utils.h

logSpeed.o: logSpeed.c logSpeed.h

shmem.o: shmem.c shmem.h

aioOperations.o: aioOperations.c aioOperations.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

clean:
	- rm -f readSpeedTest writeSpeedTest writePairTest logSpeedTest *.o \
	mtReadSpeedTest mtWriteSpeedTest checkDisks aioReadTest aioRWTest \
	flushTestReader flushTestWriter writeSmallBlocks


