
all: blockdevreplay blockdevdiff

blockdevdiff: blockdevdiff.c
	gcc -Wall -pedantic $< -o $@

blockdevreplay: blockdevreplay.c
	gcc -Wall -pedantic $< -o $@


clean:
	/bin/rm -f blockdevdiff blockdevreplay
