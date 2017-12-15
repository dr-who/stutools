GIT_VERSION := $(shell git describe --abbrev=6 --dirty --always --tags)
CC=gcc
CFLAGS=-Wall --std=c99 -Werror -DVERSION=\"$(GIT_VERSION)\" -g -O2

%.o : %.c %.h
	$(CC) -c $(CFLAGS) $< -o $@

all: readSpeedTest writeSpeedTest writePairTest checkDisks flushTestWriter flushTestReader \
	aioRWTest watchDisks flipBits indiDisks trim

readSpeedTest: readSpeedTest.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

watchDisks: watchDisks.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

flipBits: flipBits.o utils.o logSpeed.o 
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

trim: trim.o utils.o logSpeed.o
	gcc $^ -o $@ -lm 

indiDisks: indiDisks.o utils.o logSpeed.o aioOperations.o
	gcc $^ -o $@ -lpthread -lm -laio

flushTestWriter: flushTestWriter.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm 

flushTestReader: flushTestReader.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

aioRWTest: aioRWTest.o aioRequests.o logSpeed.o utils.o diskStats.o positions.o
	gcc $^ -o $@ -lm -laio

positions.o: positions.c utils.h

logSpeed.o: logSpeed.c logSpeed.h utils.h

diskStats.o: diskStats.c diskStats.h utils.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

test:
	time ./test.sh $(DRIVE)

clean:
	git clean -xdf
