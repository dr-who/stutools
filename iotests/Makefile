GIT_VERSION := $(shell git describe --abbrev=6 --dirty --always --tags)
CC=gcc
CFLAGS=-Wall --std=c99 -Werror -DVERSION=\"$(GIT_VERSION)\" -O2

all: readSpeedTest writeSpeedTest writePairTest 

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


utils.o: utils.c utils.h

logSpeed.o: logSpeed.c logSpeed.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

clean:
	- rm -f readSpeedTest writeSpeedTest writePairTest logSpeedTest *.o \
	mtReadSpeedTest mtWriteSpeedTest


