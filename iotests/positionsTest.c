#define _DEFAULT_SOURCE

#include "positions.h"
#include <stdlib.h>
#include <malloc.h>
#include "utils.h"

#include <assert.h>

int verbose = 0;
int keepRunning = 0;
int passed = 0;

void pass() {
  passed++;
  fprintf(stderr,"*test* test %d passed\n", passed);
}
  

int main() {

  srand48(0);
  
  positionType *p = createPositions(0);
  assert(p);
  freePositions(p);
  pass();

  p = createPositions(1);
  assert(p);
  assert(p[0].fd == 0);
  assert(p[0].pos == 0);
  assert(p[0].action == 0);
  assert(p[0].len == 0);
  assert(p[0].success == 0);
  freePositions(p);
  pass();

  p = createPositions(-1);
  pass(); // shouldn't fail, but should warn
  freePositions(p);

  p = createPositions(2);
  p[0].pos = 1;
  p[0].len = 1;
  p[1].pos = 1;
  p[1].len = 1;
  int ret = checkPositionArray(p, 2, 1);
  assert(ret > 0);
  pass();

  p[0].pos = 0;
  p[0].len = 2;
  ret = checkPositionArray(p, 2, 1);
  assert(ret > 0);
  pass();

  p[0].len = 1;
  ret = checkPositionArray(p, 2, 2);
  assert(ret == 0);
  pass();

  p[0].pos = 1;
  p[0].len = 1;
  ret = checkPositionArray(p, 2, 2);
  assert(ret == 0);
  pass();

  p[0].pos = 0; p[0].len = 1;
  p[1].pos = 0; p[1].len = 1;

  ret = checkPositionArray(p, 2, 1);
  assert(ret == 0);
  pass();

  ret = checkPositionArray(p, 2, 0);
  assert(ret > 0);
  pass();

  p[0].pos = 0; p[0].len = 0;
  p[1].pos = 0; p[1].len = 1;

  ret = checkPositionArray(p, 2, 100);
  assert(ret > 0);
  pass();

  freePositions(p);

  p = createPositions(3);

  p[0].pos = 0; p[0].len = 1024;
  p[1].pos = 0; p[1].len = 4096;
  p[2].pos = 0; p[2].len = 3333;

  ret = checkPositionArray(p, 3, 100000);
  assert(ret > 0);
  pass();

  // check positions not aligned
  p[0].pos = 1024; p[0].len = 1024;
  p[1].pos = 2048; p[1].len = 4096;
  p[2].pos = 1025; p[2].len = 1024;

  ret = checkPositionArray(p, 3, 100000);
  assert(ret > 0);
  pass();


  // check positions not aligned
  p[0].pos = 0; p[0].len = 1024;
  p[1].pos = 2048; p[1].len = 4096;
  p[2].pos = 1025; p[2].len = 1024;

  ret = checkPositionArray(p, 3, 100000);
  assert(ret > 0);
  pass();

    // check positions not aligned
  p[0].pos = 0; p[0].len = 1024;
  p[1].pos = 2048; p[1].len = 4096;
  p[2].pos = 1024; p[2].len = 4097;

  ret = checkPositionArray(p, 3, 100000);
  assert(ret > 0);
  pass();


      // check positions not aligned
  p[0].pos = 0; p[0].len = 1023;
  p[1].pos = 2048; p[1].len = 4096;
  p[2].pos = 1024; p[2].len = 4097;

  ret = checkPositionArray(p, 3, 100000);
  assert(ret > 0);
  pass();
  freePositions(p);

  size_t num = 100000;
  p = createPositions(num);
  assert(p); pass();

  int *fdArray = NULL;
  CALLOC(fdArray, 1, sizeof(int));
  assert(fdArray);

  size_t bdSize = 1024*1024;
  
  simpleSetupPositions(p, &num, fdArray, 1, 0, bdSize, 1<<10);
  assert(num == 1023); // last block
  //  fprintf(stderr,"num %zd\n", num);
  for (size_t i = 0; i < num; i++) {
    //    fprintf(stderr,"%zd, %zd\n", i, p[i].pos);
    assert(p[i].pos == i * (1<<10));
  }
  pass();

  size_t start = 1;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, 1<<10);
  for (size_t i = 0; i < num; i++) {
    //        fprintf(stderr,"%zd, %zd\n", i, p[i].pos);
    assert(p[i].pos == (i+start) * (1<<10));
  }
  pass();

  start = -1;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, 1<<10);
  for (size_t i = 0; i < num; i++) {
    //    fprintf(stderr,"%zd, %zd\n", i, p[i].pos);
    assert(p[i].pos == (((i+start) * (1<<10)) + bdSize) % bdSize);
  }
  pass();

  start = -99999;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, 1<<10);
  start = 392;
  for (size_t i = 0; i < num; i++) {
    //fprintf(stderr,"%zd, %zd\n", i, p[i].pos);
    assert(p[i].pos == (((i+start) * (1<<10)) + bdSize) % bdSize);
  }
  pass();




  return 0;
}
