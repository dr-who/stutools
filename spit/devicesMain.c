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
#include "keyvalue.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  
  DIR *d;
  struct dirent *dir;
  struct stat st;
  char path[1024];
  int count = 0;

  d = opendir("/sys/block/");
  if (d) {
    printf("[\n");
    while ((dir = readdir(d)) != NULL) {
      snprintf(path, sizeof(path), "/dev/%s", dir->d_name);

      if (stat(path, &st) == 0) {
	if (st.st_mode | S_IFBLK) {
	  int fd = open(path, O_RDONLY);
	  if (fd) {
	    keyvalueType *k = keyvalueInit();

	    char *suf = getSuffix(path);
	    char isr[100];
	    sprintf(isr,"/sys/block/%s/queue/rotational", suf);
	    int rot = getValueFromFile(isr, 1);
	    if (rot) {
	      keyvalueSetString(k, "type", "HDD");
	    } else {
	      keyvalueSetString(k, "type", "SSD");
	    }

	    if (getWriteCache(suf) == 0) {
	      keyvalueSetString(k, "writeCache", "write-back");
	    } else {
	      keyvalueSetString(k, "writeCache", "write-thru");
	    }
		  
	    //
	    free(suf);

		  
	    unsigned int major = 0,minor = 0;
	    majorAndMinor(fd, &major, &minor);
	    keyvalueSetString(k, "paths", path);
	    keyvalueSetLong(k, "major", major);
	    keyvalueSetLong(k, "minor", minor);
	    if (major == 8) { // HDD
		    
	      size_t s = blockDeviceSizeFromFD(fd);
	      keyvalueSetLong(k, "size", s);
	      char *serial = serialFromFD(fd);
	      keyvalueSetString(k, "serial", serial);
	      char *model = modelFromFD(fd);
	      keyvalueSetString(k, "model", model);
	      char *scsi= SCSISerialFromFD(fd);
	      keyvalueSetString(k, "scsi", scsi);

	      if (count++ >= 1) printf(",");
	      printf("%s", keyvalueDumpAsJSON(k));
	      fprintf(stderr,"%s\n", keyvalueDumpAsString(k));
	      free(serial);
	      free(model);
	      free(scsi);
	    }
	  }
	  close(fd);
	}
      }
    }
    closedir(d);
    printf("]\n");
  }

  return 0;
}

