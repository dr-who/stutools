#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <dirent.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <sys/sysmacros.h>
#include <fcntl.h>
#include <sys/types.h>
#include <linux/types.h>

#include "json.h"

#include "blockdevice.h"


#define PATH_MAX 1024

char *getFieldFromUdev(unsigned int major, unsigned int minor, char *match) {
    char s[PATH_MAX];
    sprintf(s, "/run/udev/data/b%u:%u", major, minor);
    FILE *fp = fopen(s, "rt");
    if (!fp) {
      //    perror(s);
        //    exit(1);
        return NULL;
    }

    char *line = calloc(PATH_MAX, 1);
    char *result = calloc(PATH_MAX, 1);
    while (fgets(line, PATH_MAX - 1, fp) != NULL) {
        if (strstr(line, match)) {
            sscanf(strstr(line, "=") + 1, "%s", result);
            //      fprintf(stderr,"matched: %s\n", result);
        }
    }
    fclose(fp);
    free(line);

    return result;
}


/* creates a new string */
char *getSuffix2(const char *path) {
    if (!path) return NULL;
    int found = -1;
    for (size_t i = strlen(path) - 1; i > 0; i--) {
        if (path[i] == '/') {
            found = i + 1;
            break;
        }
    }
    if (found > 0) {
        return strdup(path + found);
    } else {
        return NULL;
    }
}


long getLongValueFromFile(const char *filename) {
  FILE *fp = fopen (filename, "rt");
  long value = 0; // if can't open file etc
  if (fp) {
    int ret = fscanf(fp, "%ld", &value);
    if (ret < 1)
      return 0;

    fclose(fp);
  } else {
    //    perror(filename);
  }
  return value;
}
char * getStringFromFile2(const char *filename) {
  int fd = open(filename, O_RDONLY); 
  char str[1024];
  memset(str, 0, 1024);
  if (fd >= 0) {
    int ret = read(fd, str, 1023);
    //    int ret = fscanf(fp, "%s", str);
    if (ret < 1)
      return NULL;
    str[ret] = 0; // add end 0
    str[strlen(str)-1] = 0; // remove last \n
    close(fd);
  } else {
    //    perror(filename);
  }
  return strdup(str);
}

int majorAndMinor(int fd, unsigned int *major, unsigned int *minor) {
    struct stat buf;
    if (fstat(fd, &buf) == 0) {
        dev_t dt = buf.st_rdev;
        *major = major(dt);
        *minor = minor(dt);
        return 0;
    }
    return 1;
}



int majorAndMinorFN(char *fn, unsigned int *major, unsigned int *minor) {
  FILE *fp = fopen(fn, "rt");
  if (fp) {
    fscanf(fp, "%u:%u", major, minor);
    fclose(fp);
    return 0;
  }
  return 1;
}


bdType *bdInit(const char *path) {
  bdType *b = calloc(1, sizeof(bdType));
  b->path = strdup(path);

  char *suffix = getSuffix2(path);

  char s[1024];

  //  int fd = open(path, O_RDONLY);
  //  if (fd >= 0) {
    sprintf(s, "/sys/block/%s/dev", suffix);
    majorAndMinorFN(s, &b->major, &b->minor);

    if (b->major == 0) {
      return NULL;
    }
    
    if (b->major == 253) {
      b->mapper = 1;
    }
    
    sprintf(s, "/sys/block/%s", suffix);
    int fd2 = open(s, O_RDONLY|O_DIRECTORY);
    if (fd2 < 0) {
      fprintf(stderr,"*warning* '%s' is not a block device\n", path);
      return NULL;
    } else {
      close(fd2);
    }


    sprintf(s, "/sys/block/%s/partition", suffix);
    fd2 = open(s, O_RDONLY);
    if (fd2 >= 0) {
      b->partition = 1;
      close(fd2);
      fprintf(stderr,"*warning* '%s' is not a block device, it's a partition\n", path);
      return NULL;
    }

    b->serial = getFieldFromUdev(b->major, b->minor, "E:ID_SERIAL_SHORT=");
    b->vendor = getFieldFromUdev(b->major, b->minor, "E:ID_VENDOR=");
    b->model = getFieldFromUdev(b->major, b->minor, "E:ID_MODEL=");

    b->usbdriver = getFieldFromUdev(b->major, b->minor, "E:ID_USB_DRIVER=");
    
    sprintf(s, "/sys/block/%s/rotational", suffix);
    b->rotational = getLongValueFromFile(s);

    sprintf(s, "/sys/block/%s/removable", suffix);
    b->removable = getLongValueFromFile(s);
    if (b->usbdriver) {
      if ((strcmp(b->usbdriver, "uas")==0) || (strcmp(b->usbdriver,"usb-storage")==0))
	b->removable = 1;
    }
	

    sprintf(s, "/sys/block/%s/queue/write_cache", suffix);
    b->writecache = getStringFromFile2(s);
	
    sprintf(s, "/sys/block/%s/size", suffix);
    b->size = 512LL * getLongValueFromFile(s);

    if (b->removable) {
      b->type = strdup("Removeable");
    } else {
      if (b->major == 1) {
	b->type = strdup("Volatile-RAM");
      } else if (b->major == 8) {
	if (b->rotational) {
	  b->type = strdup("HDD");
	} else {
	  b->type = strdup("SSD");
	}
      } else if (b->major == 259) {
	b->type = strdup("NVMe");
      } else if (b->major == 253) {
	b->type = strdup("Device-Mapper");
      } else if (b->major >= 243) {
	b->type = strdup("Virtual");
      } else {
	b->type = strdup("Unknown");
      }
    }
    
      
    //  } else {
    //    perror(path);
    //    fprintf(stderr,"*warning* '%s' is not a block device2\n", path);
    //    return NULL;
    //  }
  
  return b;
}



void bdFree(bdType **bdin) {
  if (bdin) {
    bdType *bd = *bdin;
    if (bd) {
      free(bd->path);
      free(bd->vendor);
      free(bd->model);
      free(bd->serial);
      free(bd->usbdriver);
      free(bd->writecache);
      free(bd->type);
    }
    free(bd);
    *bdin = NULL;
  }
}
    

jsonValue *bdJsonValue(bdType *bd) {
  if (bd) {
    jsonValue *node = jsonValueObject();
    jsonObjectAdd(node, "Path", jsonValueString(bd->path));
    jsonObjectAdd(node, "Type", jsonValueString(bd->type));
    jsonObjectAdd(node, "Partition", jsonValueNumber(bd->partition));
    jsonObjectAdd(node, "Major", jsonValueNumber(bd->major));
    jsonObjectAdd(node, "Minor", jsonValueNumber(bd->minor));
    jsonObjectAdd(node, "DeviceMapper", jsonValueNumber(bd->mapper));
    jsonObjectAdd(node, "USBDriver", jsonValueString(bd->usbdriver));
    jsonObjectAdd(node, "SizeBytes", jsonValueNumber(bd->size));
    jsonObjectAdd(node, "SizeGiB", jsonValueNumber(bd->size/1024.0/1024/1024));
    jsonObjectAdd(node, "Serial", jsonValueString(bd->serial));
    jsonObjectAdd(node, "Vendor", jsonValueString(bd->vendor));
    jsonObjectAdd(node, "Model", jsonValueString(bd->model));
    jsonObjectAdd(node, "Removable", jsonValueNumber(bd->removable));
    jsonObjectAdd(node, "Volatile", jsonValueNumber(bd->major == 1));
    jsonObjectAdd(node, "WriteCache", jsonValueString(bd->writecache));
    return node;
  }
  return NULL;
}

jsonValue * bdScan(void) {
  DIR *d;
  struct dirent *dir;
  struct stat st;
  char path[1024];

  jsonValue *j = jsonValueArray();
  
  d = opendir("/sys/block/");
  if (d) {
    while ((dir = readdir(d)) != NULL) {
      snprintf(path, sizeof(path), "/dev/%s", dir->d_name);

      if (strchr(path, '.'))
	continue;
      
      if (stat(path, &st) == 0) {
	if (st.st_mode | S_IFBLK) {
	  //	  fprintf(stderr,"%s\n", path);
	  bdType *bd = bdInit(path);
	  jsonArrayAdd(j, bdJsonValue(bd));
	}
	//	jsonValueDump(j);
      }
    }
  }

  return j;
}
