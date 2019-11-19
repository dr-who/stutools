#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include "utils.h"
#include "devices.h"
#include <linux/hdreg.h>
#include <sys/ioctl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <stdlib.h>

int verbose = 0;
int keepRunning = 1;

int main( int argc, char* argv[] ) {
  printf("DEVICE\t\tIS_BD\tSIZE(GiB)\tIN_USE\n");//\tW/CACHE\n");
  for (int i = 1; i < argc; ++i ) {
    const char* bd_path = argv[ i ];
    int is_bd = isBlockDevice(bd_path);
    double size = TOGiB(blockDeviceSize(bd_path));
    int in_use = !canOpenExclusively(bd_path);
    // TODO
    /*
    int write_cache = -1;
    int fd = open(bd_path, O_RDWR | O_DIRECT, S_IRUSR | S_IWUSR );
    if(fd>0) {
      
      close(fd);
    }
    */
    printf("%s\t%d\t%0.2f\t\t%d\t\n", bd_path, is_bd, size, in_use);
  }
  return 0;
}
