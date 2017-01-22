all: readSpeedTest

readSpeedTest:
	gcc --std=c11 -O2 readSpeedTest.c -o readSpeedTest -lpthread -lm

writeSpeedTest:
	gcc --std=c11 -O2 writeSpeedTest.c -o writeSpeedTest -lpthread -lm

clean:
	- rm readSpeedTest
	- rm writeSpeedTest
