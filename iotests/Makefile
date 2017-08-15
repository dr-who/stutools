GIT_VERSION := $(shell git describe --abbrev=6 --dirty --always --tags)
CC=gcc
CFLAGS=-Wall --std=c99 -Werror -DVERSION=\"$(GIT_VERSION)\" -g -O2

%.o : %.c %.h
	$(CC) -c $(CFLAGS) $< -o $@

all: readSpeedTest writeSpeedTest writePairTest checkDisks flushTestWriter flushTestReader \
	aioRWTest

readSpeedTest: readSpeedTest.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

writeSpeedTest: writeSpeedTest.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

writePairTest: writePairTest.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

mtWriteSpeedTest: mtWriteSpeedTest.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

mtReadSpeedTest: mtReadSpeedTest.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

checkDisks: checkDisks.o utils.o logSpeed.o aioOperations.o
	gcc $^ -o $@ -lpthread -lm -laio

flushTestWriter: flushTestWriter.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm 

flushTestReader: flushTestReader.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

aioRWTest: aioRWTest.o aioRequests.o logSpeed.o utils.o diskStats.o 
	gcc $^ -o $@ -lm -laio


logSpeed.o: logSpeed.c logSpeed.h utils.h

diskStats.o: diskStats.c diskStats.h utils.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

clean:
	- rm -f readSpeedTest writeSpeedTest writePairTest logSpeedTest *.o \
	mtReadSpeedTest mtWriteSpeedTest checkDisks aioReadTest aioRWTest \
	flushTestReader flushTestWriter writeSmallBlocks


