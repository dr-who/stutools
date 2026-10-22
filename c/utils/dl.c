#include <stdio.h>
#include <sys/types.h>
#include <dirent.h>



void list(char *path) {
  struct dirent *ep;

  DIR *dp = opendir(path);
  if (dp != NULL) {
    while ((ep = readdir (dp))) {
      printf("%s/%s %d\n", path, ep->d_name, ep->d_type);
      if ((ep->d_type == DT_DIR) && (ep->d_name[0] != '.')) {
	char s[1000];
	sprintf(s, "%s/%s", path, ep->d_name);
	//	printf("recursively going into %s\n", s);
	list(s);
      }
    }
    closedir (dp);
  } else {
    perror(path);
  }
}
    
  


int
main (void)
{
  list("/");

  return 0;
}
