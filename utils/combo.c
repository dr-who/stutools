#define _POSIX_C_SOURCE 200809L
  
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*

combo

q1,q8,q128 r,w k4P-100000

r,w q1,q4,q128 k4s0,k4P.10000,k4P-10000,k4P+10000 -% -g 0-10,90-100

For each column C, create N values. C[0..N-1]

Print all combinations with a templated string

*/

typedef struct {
  size_t num;
  char **value;
  size_t printcount;
} itertype;


void display(int argc, char *argv[], itertype *list, size_t cols) {
  int curcol = cols - 1;
  while (1) {

    for (size_t i = 1; i < argc; i++) {
      printf("%s ", argv[i]);
    }
    
    for (size_t i = 0; i < cols; i++) {
      int slash = (strlen(list[i].value[list[i].printcount]) >= 2) && (list[i].value[list[i].printcount][0] == '^');
      if ((i > 0) && (!slash)) {
	printf(" ");
      }
      if (slash) {
	printf("%s", list[i].value[list[i].printcount] + 1);
      } else {
	printf("%s", list[i].value[list[i].printcount]);
      }
    }
    printf("\n"); // printed a line
    list[curcol].printcount++;
    
    if (list[curcol].printcount >= list[curcol].num) {
      // if current column is too high, reset
	// find one to the left that we can increase
      while (--curcol >= 0) {
	if (list[curcol].printcount < list[curcol].num-1) { // can increase it
	  list[curcol].printcount++;
	  // make all zeros
	  for (int j = curcol+1; j < cols; j++) {
	    list[j].printcount = 0;
	  }
	  curcol = cols-1;
	  break;
	}
      }
      if (curcol < 0) break;
    }
  }
}
  

int main(int argc, char *argv[]) {

  /*  if (argc <= 1) {
    fprintf(stderr,"combo prefix*  < combinations.txt\n");
    exit(1);
    }*/

  FILE *stream = stdin;
  
  char *line = NULL;
  size_t len = 0;
  int nread;

  
  while ((nread = getline(&line, &len, stream)) != -1) {
    if ((strlen(line) > 1) && (line[0] != '#')) {
      size_t cols = 0;
      itertype *list = NULL;
      //      printf("line: %s", line);
      char *tok = NULL, *toksave = NULL;

      char *linecopy = strdup(line);
      
      while ((tok = strtok_r(tok==NULL?linecopy:NULL, " \n", &toksave))) {

	list = realloc(list, sizeof(itertype) * (cols+1));
	list[cols].num = 0;
	list[cols].printcount = 0;
	list[cols].value = NULL;
	
	char *copy = strdup(tok);
	
	// we have a valid token
	char *tok2 = NULL, *tok2save = NULL;
	int ln = 0;
	
	while ((tok2 = strtok_r(tok2 == NULL?copy: NULL, ",", &tok2save))) {
	  ln = list[cols].num;
	  list[cols].value = realloc(list[cols].value, sizeof(char*) * (ln+1));
	  list[cols].value[ln] = tok2;
	  list[cols].num++;
	}
	
	cols++;
      } // toik the line


      display(argc, argv, list, cols);
    }
  }

  exit(0);
}









