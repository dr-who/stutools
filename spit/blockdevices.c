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

#include "blockdevices.h"


blockDevicesType * blockDevicesInit(void) {
  blockDevicesType *p = calloc(1, sizeof(blockDevicesType)); assert(p);

  return p;
}

// clobber based on serial
void blockDevicesAddKV(blockDevicesType *bd, keyvalueType *kv) {
  // first search
  size_t match = bd->num;
  for (size_t i = 0; i < bd->num; i++) {
    if (strcmp(keyvalueGetString(bd->devices[i].kv, "serial"), keyvalueGetString(kv, "serial")) == 0) {
      match = i;
    }
  }
  if (match >= bd->num) {
    // not there
    bd->num++;
    bd->devices = realloc(bd->devices, bd->num * sizeof(blockDevicesType)); assert(bd->devices);
  }
  bd->devices[match].kv = kv;
}

void blockDevicesScan(blockDevicesType *bd) {
    DIR *d;
    struct dirent *dir;
    struct stat st;
    char path[1024];

    d = opendir("/sys/block/");
    if (d) {
        while ((dir = readdir(d)) != NULL) {
            snprintf(path, sizeof(path), "/dev/%s", dir->d_name);

            if (stat(path, &st) == 0) {
	      if (st.st_mode | S_IFBLK) {
		int fd = open(path, O_RDONLY);
		if (fd >= 0) {
		  keyvalueType *k = keyvalueInit();

		  char *suf = getSuffix(path);
		  char isr[100];
		  sprintf(isr,"/sys/block/%s/queue/rotational", suf);

		  int rot = getValueFromFile(isr, 1);

		  char isremove[100];
		  sprintf(isremove,"/sys/block/%s/removable", suf);

		  int canremove = getValueFromFile(isremove, 1);

		  if (canremove) {
		    keyvalueSetString(k, "type", "Removable");
		  } else {
		    if (rot && 0) {
		      keyvalueSetString(k, "type", "HDD");
		    } else {
		      keyvalueSetString(k, "type", "SSD");
		    }
		  }

		  //		  /sys/block/dev name/queue/chunk_sectors
		  sprintf(isr,"/sys/block/%s/queue/zoned", suf);
		  char *zoned = getStringFromFile(isr, 1);
		  if (zoned) keyvalueSetString(k, "zoned", zoned);

		  if (zoned && (strcmp(zoned, "none")!=0)) {
		    sprintf(isr,"/sys/block/%s/queue/chunk_sectors", suf);
		    int chunk  = getValueFromFile(isr, 1);
		    if (chunk) 
		      keyvalueSetLong(k, "zonesize", chunk);
		  }
		  free(zoned);


		  if (getWriteCache(suf) == 0) {
		    keyvalueSetString(k, "writeCache", "write-back");
		  } else {
		    keyvalueSetString(k, "writeCache", "write-thru");
		  }
		  
		  //
		  free(suf);

		  
		  unsigned int major = 0,minor = 0;
		  majorAndMinor(fd, &major, &minor);
		  keyvalueAddString(k, "paths", path);
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


		    blockDevicesAddKV(bd, k);

		    free(serial);
		    free(model);
		    free(scsi);
		  }
		} // fd
		close(fd);
	      }
	    }
	}
        closedir(d);
    }
}  


char * blockDevicesAllJSON(blockDevicesType *bd) {
  char *ret = calloc(1000000+1,1); assert(ret);
  for (size_t i = 0; i < bd->num; i++) {
    char *s = keyvalueDumpAsJSON(bd->devices[i].kv);
    strncat(ret, s, 1000000);
  }
  char *toret=strdup(ret);
  free(ret);
  return toret;
}
  


void blockDevicesFree(blockDevicesType *bd) {
  for (size_t i = 0 ; i < bd->num; i++) {
    keyvalueFree(bd->devices[i].kv);
  }
  free(bd->devices);
  free(bd);
}

 // SSD, HDD type
size_t blockDevicesCount(blockDevicesType *bd, const char *devtype, size_t *sumbytes) {
  size_t matched = 0;
  (*sumbytes) = 0;
  for (size_t i = 0; i < bd->num; i++) {
    char *v = keyvalueGetString(bd->devices[i].kv, "type");
    if (v) {
      if ((devtype == NULL) || (strcmp(v, devtype) == 0)) {
	matched++;
	*sumbytes = (*sumbytes) + keyvalueGetLong(bd->devices[i].kv, "size");
      }
    }
  }
  return matched;
}
	
