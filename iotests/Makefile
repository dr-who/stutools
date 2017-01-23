CC=gcc
CFLAGS=-Wall --std=c99 -O2

all: readSpeedTest writeSpeedTest writePairTest 

readSpeedTest: readSpeedTest.c utils.o logSpeed.o
	gcc -Wall --std=c99 -O2 $^ -o $@ -lpthread -lm

writeSpeedTest: writeSpeedTest.c utils.o logSpeed.o
	gcc -Wall --std=c99 -O2 $^ -o $@ -lpthread -lm

writePairTest: writePairTest.o utils.o logSpeed.o
	gcc -Wall --std=c99 -O2 $^ -o $@ -lpthread -lm

utils.o: utils.c utils.h

logSpeed.o: logSpeed.c logSpeed.h

logSpeedTest: logSpeedTest.o utils.o logSpeed.o

clean:
	- rm -f readSpeedTest writeSpeedTest writePairTest logSpeedTest *.o


