#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <sys/sysmacros.h>
#include <fcntl.h>

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
char *getSuffix(const char *path) {
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
char * getStringFromFile(const char *filename) {
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


bdType *bdInit(const char *path) {
  bdType *b = calloc(1, sizeof(bdType));
  b->path = strdup(path);

  int fd = open(path, O_RDONLY);
  if (fd >= 0) {
    majorAndMinor(fd, &b->major, &b->minor);
    if (b->major == 253) {
      b->mapper = 1;
    }
    
    char *suffix = getSuffix(path);
    char s[1024];
    sprintf(s, "/sys/class/block/%s", suffix);
    int fd2 = open(s, O_RDONLY|O_DIRECTORY);
    if (fd2 < 0) {
      fprintf(stderr,"*warning* '%s' is not a block device\n", path);
      return NULL;
    } else {
      close(fd2);
    }


    sprintf(s, "/sys/class/block/%s/partition", suffix);
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
    
    sprintf(s, "/sys/class/block/%s/rotational", suffix);
    b->rotational = getLongValueFromFile(s);

    sprintf(s, "/sys/class/block/%s/removable", suffix);
    b->removable = getLongValueFromFile(s);
    if (b->usbdriver) {
      if ((strcmp(b->usbdriver, "uas")==0) || (strcmp(b->usbdriver,"usb-storage")==0))
	b->removable = 1;
    }
	

    sprintf(s, "/sys/class/block/%s/queue/write_cache", suffix);
    b->writecache = getStringFromFile(s);
	
    sprintf(s, "/sys/class/block/%s/size", suffix);
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
      } else if (b->major >= 243) {
	b->type = strdup("Virtual");
      } else {
	b->type = strdup("Unknown");
      }
    }
    
      
  } else {
    fprintf(stderr,"*warning* '%s' is not a block device\n", path);
    return NULL;
  }
  
  return b;
}

void bdDumpKV(FILE *fp, bdType *bd) {
  fprintf(fp, "path: %s\n", bd->path);
  fprintf(fp, "type: %s\n", bd->type);
  fprintf(fp, "  partition: %d\n", bd->partition);
  fprintf(fp, "maj:min: %u:%u\n", bd->major, bd->minor);
  fprintf(fp, "device mapper: %d\n", bd->mapper);
  fprintf(fp, "USB driver: %s\n", bd->usbdriver);
  fprintf(fp, "sizeBytes: %lld\n", bd->size);
  fprintf(fp, "sizeGiB: %.1lf\n", bd->size/1024.0/1024);
  fprintf(fp, "serial: %s\n", bd->serial);
  fprintf(fp, "vendor: %s\n", bd->vendor);
  fprintf(fp, "model: %s\n", bd->model);
  fprintf(fp, "removable: %d\n", bd->removable);
  fprintf(fp, "volatile: %d\n", (bd->major == 1));
  fprintf(fp, "write cache: %s\n", bd->writecache);
}

