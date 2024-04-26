#include <stdio.h>
#include <dirent.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <stdio.h>
#include <sys/types.h>
#include <linux/types.h>
#include <sys/stat.h>
#include <sys/ioctl.h>
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
    char *s1 = keyvalueGetString(bd->devices[i].kv, "serial");
    char *s2 = keyvalueGetString(kv, "serial");
    if (s1 && s2 && (strcmp(s1, s2) == 0)) {
      match = i;
    }
    free(s1);
    free(s2);
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
		  fprintf(stderr,"%s\n", path);
		  keyvalueType *k = keyvalueInit();

		  char *suf = getSuffix(path);
		  char isr[100];
		  sprintf(isr,"/sys/block/%s/queue/rotational", suf);


		  unsigned int major = 0,minor = 0;
		  majorAndMinor(fd, &major, &minor);
		  keyvalueSetString(k, "paths", path);
		  keyvalueSetLong(k, "major", major);
		  keyvalueSetLong(k, "minor", minor);


		  fprintf(stderr,"  m %d %d\n", major, minor);
		  
		  int rot = getValueFromFile(isr, 1);

		  char isremove[100];
		  sprintf(isremove,"/sys/block/%s/removable", suf);

		  int canremove = getValueFromFile(isremove, 1);

		  if (canremove) {
		    keyvalueSetString(k, "type", "Removable");
		  } else {
		    if (major == 1) { // ram
			keyvalueSetString(k, "type", "Volatile-RAM");
		    } else if (major == 8) {
		      if (rot) {
			keyvalueSetString(k, "type", "HDD");
		      } else {
			keyvalueSetString(k, "type", "SSD");
		      }
		    } else if (major == 259) {
		      keyvalueSetString(k, "type", "NVMe");
		    } else if (major >= 243) {
		      keyvalueSetString(k, "type", "Virtual");
		    } else {
		      fprintf(stderr,"issue!\n");
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

		  
		  if (major >= 243 || major ==8 || major == 1) {
		    // valid one to add
		    char *serial = NULL, *model = NULL, *vendor = NULL, *scsi = NULL;
		    size_t s = blockDeviceSizeFromFD(fd);
		    keyvalueSetLong(k, "size", s);
		    if (major >= 243) {
		    } 
		    if (major == 8) { // block
		      
		      serial = serialFromFD(fd);
		      keyvalueSetString(k, "serial", serial?serial:"n/a");
		      model = modelFromFD(fd);
		      keyvalueSetString(k, "model", model?model:"n/a");
		      vendor = vendorFromFD(fd);
		      keyvalueSetString(k, "vendor", vendor?vendor:"n/a");
		      scsi= SCSISerialFromFD(fd);
		      keyvalueSetString(k, "scsi", scsi?scsi:"n/a");

		    }
		    
		    blockDevicesAddKV(bd, k);
		    free(serial);
		    free(model);
		    free(scsi);
		    
		  } else {
		    keyvalueFree(k);
		  }
		} // fd
		close(fd);
	      }
	    }
	}
        closedir(d);
    }
}  

char * blockDeviceFromSerial(blockDevicesType *bd, const char *serial) {
  for (size_t i = 0; i < bd->num; i++) {
    if (strcmp(keyvalueGetString(bd->devices[i].kv, "serial"), serial) == 0) {
      return keyvalueGetString(bd->devices[i].kv, "paths");
    }
  }
  return NULL;
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
    bd->devices[i].kv = NULL;
  }
  free(bd->devices);
  bd->devices = NULL;
  free(bd);
}

 // SSD, HDD type
size_t blockDevicesCount(blockDevicesType *bd, const char *devtype, size_t *sumbytes) {
  size_t matched = 0;
  (*sumbytes) = 0;
  for (size_t i = 0; i < bd->num; i++) {
    char *v = keyvalueGetString(bd->devices[i].kv, "type");
    //    fprintf(stderr,"checking haystack %s, needle %s\n", devtype, v);
    if (v) {
      if ((devtype == NULL) || (strstr(devtype, v) != 0)) {
	//	fprintf(stderr,"match\n");
	matched++;
	*sumbytes = (*sumbytes) + keyvalueGetLong(bd->devices[i].kv, "size");
      }
      free(v);
    }
  }
  return matched;
}
	
