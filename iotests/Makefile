all: readSpeedTest writeSpeedTest writeMatrixTest

readSpeedTest: readSpeedTest.c
	gcc -Wall --std=c99 -O2 $^ -o $@ -lpthread -lm

writeSpeedTest: writeSpeedTest.c
	gcc -Wall --std=c99 -O2 $^ -o $@ -lpthread -lm

writeMatrixTest: writeMatrixTest.c
	gcc -Wall --std=c99 -O2 $^ -o $@ -lpthread -lm

clean:
	- rm -f readSpeedTest
	- rm -f writeSpeedTest
	- rm -f writeMatrixTest

