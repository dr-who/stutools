#include <stdio.h>
#include <dirent.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

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
	    printf("path: %s\n", path);

            if (stat(path, &st) == 0) {
	      switch (st.st_mode) {
	      case S_IFBLK:  printf("block device\n");            break;
	      case S_IFCHR:  printf("character device\n");        break;
	      case S_IFDIR:  printf("directory\n");               break;
	      case S_IFIFO:  printf("FIFO/pipe\n");               break;
	      case S_IFLNK:  printf("symlink\n");                 break;
	      case S_IFREG:  printf("regular file\n");            break;
	      case S_IFSOCK: printf("socket\n");                  break;
	      default:       printf("unknown?\n");                break;
	      }

                printf("reg: %s\n", dir->d_name);
	    }
	}
        closedir(d);
    }

    return 0;
}

