all: readSpeedTest

readSpeedTest: readSpeedTest.c
	gcc -Wall --std=c11 -O2 $^ -o $@ -lpthread -lm

writeSpeedTest:
	gcc --std=c11 -O2 writeSpeedTest.c -o writeSpeedTest -lpthread -lm

clean:
	- rm readSpeedTest
	- rm writeSpeedTest
