GIT_VERSION := $(shell git describe --abbrev=6 --dirty --always --tags)
CC=gcc
CFLAGS=-Wall --std=c99 -Werror -DVERSION=\"$(GIT_VERSION)\" -g -Ofast
DRIVE=/dev/loop3

%.o : %.c %.h
	$(CC) -c $(CFLAGS) $< -o $@

all: writePairTest checkDisks flushTestWriter flushTestReader \
	aioRWTest watchDisks flipBits indiDisks trim cigarTest ramSpeedTest aioMulti \
	flushTester offTheEnds \
	positionsTest

readSpeedTest:
	@echo use aioRWTest -r

writeSpeedTest:
	@echo use aioRWTest -w

#readSpeedTest: readSpeedTest.o utils.o logSpeed.o 
#	gcc $^ -o $@ -lpthread -lm

positionsTest: positionsTest.o positions.o cigar.o utils.o
	gcc $^ -o $@ -lpthread -lm

watchDisks: watchDisks.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

offTheEnds: offTheEnds.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

flushTester: flushTester.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

flipBits: flipBits.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

#writeSpeedTest: writeSpeedTest.o utils.o logSpeed.o
#	gcc $^ -o $@ -lpthread -lm

writePairTest: writePairTest.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

mtWriteSpeedTest: mtWriteSpeedTest.o utils.o logSpeed.o
	gcc $^ -o $@ -lpthread -lm

mtReadSpeedTest: mtReadSpeedTest.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

checkDisks: checkDisks.o utils.o logSpeed.o aioRequests.o positions.o cigar.o
	gcc $^ -o $@ -lpthread -lm -laio

trim: trim.o utils.o logSpeed.o
	gcc $^ -o $@ -lm 

indiDisks: indiDisks.o utils.o logSpeed.o aioRequests.o positions.o cigar.o
	gcc $^ -o $@ -lpthread -lm -laio

flushTestWriter: flushTestWriter.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm 

flushTestReader: flushTestReader.o utils.o logSpeed.o 
	gcc $^ -o $@ -lpthread -lm

aioRWTest: aioRWTest.o aioRequests.o logSpeed.o utils.o diskStats.o positions.o cigar.o
	gcc $^ -o $@ -lm -laio

aioMulti: aioMulti.o aioRequests.o logSpeed.o utils.o diskStats.o positions.o cigar.o
	gcc $^ -o $@ -lm -laio -lpthread

positions.o: positions.c utils.h

logSpeed.o: logSpeed.c logSpeed.h utils.h

diskStats.o: diskStats.c diskStats.h utils.h

cigar.o: cigar.c cigar.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

cigarTest: cigarTest.o cigar.o
	gcc $^ -o $@ -lm

ramSpeedTest: ramSpeedTest.c utils.o logSpeed.o

test:
	./positionsTest

clean:
	git clean -xdf
	/bin/rm -f writeSpeedTest readSpeedTest

graph: aioRWTest
	./aioRWTest -f $(DRIVE) -s 1 -j0 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j0.png
	./aioRWTest -f $(DRIVE) -s 1 -j2 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j2.png
	./aioRWTest -f $(DRIVE) -s 1 -j5 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j5.png
	./aioRWTest -f $(DRIVE) -s 1 -j50 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j50.png
	./aioRWTest -f $(DRIVE) -s 5 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s5.png
	./aioRWTest -f $(DRIVE) -s 9 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s9.png
	./aioRWTest -f $(DRIVE) -s 32 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s32.png
	./aioRWTest -f $(DRIVE) -s 1024 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1024.png
	./aioRWTest -f $(DRIVE) -s -1 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s-1.png
	./aioRWTest -f $(DRIVE) -s -5 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s-5.png
	./aioRWTest -f $(DRIVE) -s -9 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s-9.png
	./aioRWTest -f $(DRIVE) -s -32 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s-32.png
	./aioRWTest -f $(DRIVE) -s -1024 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s-1024.png
	./aioRWTest -f $(DRIVE) -s 0 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s0.png
	./aioRWTest -f $(DRIVE) -s 1 -j10 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j10.png
	./aioRWTest -f $(DRIVE) -s 1 -j10 -P100 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j10p100.png
	./aioRWTest -f $(DRIVE) -s 1 -j10 -P1000 -t1 -z -L x -v && gnuplot <gnustub && /bin/mv -f a.png s1j10p1000.png
	@echo \# display "*.png" # to see the results
