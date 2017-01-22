all: readSpeedTest writeSpeedTest

readSpeedTest: readSpeedTest.c
	gcc -Wall --std=c11 -O2 $^ -o $@ -lpthread -lm

writeSpeedTest: writeSpeedTest.c
	gcc -Wall --std=c11 -O2 $^ -o $@ -lpthread -lm

clean:
	- rm -f readSpeedTest
	- rm -f writeSpeedTest
