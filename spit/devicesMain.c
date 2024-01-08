#include <stdio.h>
#include <dirent.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#include "diskStats.h"
#include "utils.h"

int keepRunning = 1;

int main() {
    DIR *d;
    struct dirent *dir;
    struct stat st;
    char path[1024];

    d = opendir("/sys/class/block/");
    if (d) {
        while ((dir = readdir(d)) != NULL) {
            snprintf(path, sizeof(path), "/dev/%s", dir->d_name);

            if (stat(path, &st) == 0) {
	      if (st.st_mode | S_IFBLK) {
		int fd = open(path, O_RDONLY);
		if (fd) {
		  unsigned int major,minor;
		  majorAndMinor(fd, &major, &minor);
		  if (major == 8) { // HDD
		    
		    size_t s = blockDeviceSizeFromFD(fd);
		    char *serial = serialFromFD(fd);
		    char *model = modelFromFD(fd);
		    printf("%-20s: ", path);
		    printf("\t %u:%u\t%zd\t%-20s\t%-20s\n", major, minor, s, model, serial);
		    free(serial);
		    free(model);
		  }
		}
		close(fd);
	      }
	    }
	}
        closedir(d);
    }

    return 0;
}

