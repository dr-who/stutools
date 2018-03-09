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

void yay() {
  fprintf(stderr,"*test* ALL TESTS PASSED.\n");
}


int main() {

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
  CALLOC(fdArray, 100, sizeof(int));
  assert(fdArray);
  for (size_t i = 0; i < 100 ;i++) { //100
    fdArray[i] = i+3;
  }

  size_t bdSize = 1024*1024;

  long start = 0;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, 1<<10);
  fprintf(stderr,"num %zd\n", num);
  assert(num == 1023); // last block
  for (size_t i = 0; i < num; i++) {
    //    fprintf(stderr,"%zd, %zd\n", i, p[i].pos);
    assert(p[i].pos == i * (1<<10));
  }
  pass();

  start = 0;

  // sequential in parallel on lots of disks
  for (size_t fdArraySize = 1; fdArraySize < 3; fdArraySize++) {
    num = 100000;
    simpleSetupPositions(p, &num, fdArray, fdArraySize, start, bdSize, 1<<10);
    assert(num == 1023 * fdArraySize); // last block
    //  fprintf(stderr,"num %zd\n", num);
    for (size_t i = 0; i < num; i++) {
      //      fprintf(stderr,"%zd, %zd, fd %d\n", i, p[i].pos, p[i].fd);
      assert(p[i].pos == (i/fdArraySize) * (1<<10));
      
      assert(p[i].fd == fdArray[i % fdArraySize]);
    }
    pass();
  }

  
  // sequential in parallel, with multiple files on multiple disks
  for (size_t fdArraySize = 2; fdArraySize < 3; fdArraySize++) {
    int sf = 3;
    num = 100000;
    int jump = 0;
    size_t bs = 1 << 10;
    int mult = sf * fdArraySize;
    
    setupPositions(p, &num, fdArray, fdArraySize, bdSize, sf, 1, bs, bs, bs, 0, jump, 0, bdSize, 0, NULL);

    assert(num == 1023 * fdArraySize ); // limit will now be num as multiple disks
    for (size_t i = 0; i < num - (sf * fdArraySize); i+= (sf*fdArraySize)) {
      //      fprintf(stderr,"%zd, %zd, fd %d\n", i, p[i].pos, p[i].fd);
      for (size_t j = 0; j < fdArraySize; j++) {
	assert(p[i + j].pos == (i/mult) * (1<<10));
      }
    }

    for (size_t i = 0; i < num - (sf * fdArraySize); i+= (sf*fdArraySize)) {
      //      fprintf(stderr,"%zd, %zd, fd %d\n", i, p[i].pos, p[i].fd);
      for (size_t j = 0; j < sf; j++) {
	assert(p[i + j * fdArraySize].pos == p[j*fdArraySize].pos + (i/mult) * (1<<10));
      }
    }

    pass();
  }

  size_t bs = 1 << 10;
  start = 1;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, 1<<10);
  for (size_t i = 0; i < num; i++) {
    size_t shouldbe = (i + start) * bs + bdSize;
    //    fprintf(stderr,"jump %d: %zd, %zd (should be %zd), pos modulo %zd\n", 0, i, p[i].pos, shouldbe, shouldbe % (bdSize));
    assert(p[i].pos == shouldbe % (bdSize));
  }
  pass();

  start = -1;
  num = 10000;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, bs);
  for (long i = 0; i < num; i++) {
    long shouldbe = 0;
    if (i + start < 0) {
      shouldbe = (i + num+1 + start) * bs;
    } else {
      shouldbe = ((i + start) % (num+1)) * bs;
    }
    //        fprintf(stderr,"jump %d: %zd, %zd (should be %zd), pos modulo %zd\n", 0, i, p[i].pos, shouldbe, shouldbe % (bdSize));
    assert(p[i].pos == (shouldbe % bdSize));
  }
  pass();


  start = 0;
  bs = 1 << 9;
  num = 100000; // big number
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, bs);
  assert(num == bdSize/bs - 1); // last block
  for (size_t i = 0; i < num; i++) {
    assert(p[i].pos == i * bs);
  }
  pass();

  bs = 1<<8; // 256
  num = 100000;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, bs);
  assert(num == bdSize/bs - 1); // last block
  for (size_t i = 0; i < num; i++) {
    assert(p[i].pos == i * bs);
  }
  pass();

  bs = 1<<18; // 256KiB
  num = 4;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, bs);
  assert(num==3);
  assert(num == bdSize/bs - 1); // last block
  for (size_t i = 0; i < num; i++) {
    assert(p[i].pos == i * bs);
  }
  pass();

  // less positions
  bs = 1<<18; // 256KiB
  num = 0;
  simpleSetupPositions(p, &num, fdArray, 1, start, bdSize, bs);
  assert(num==0);
  for (size_t i = 0; i < num; i++) {
    assert(p[i].pos == i * bs);
  }
  pass();

  // less positions
  for (size_t bits=1; bits < 20; bits++) {
    bs = 1<<bits;
    for (long startO = -100; startO < 1000; startO++) {
      num = 1;
      simpleSetupPositions(p, &num, fdArray, 1, startO, bdSize, bs);
      assert(num==1);
      for (size_t i = 0; i < num; i++) {
	assert(p[i].pos == ((i + startO) * bs + bdSize) % bdSize);
      }
    }
    pass();
  }

  bs = 1<<10; // 256KiB
  num = 10000;
  setupPositions(p, &num, fdArray, 1, bdSize, 1, 1, bs, bs, bs+1, 0, 0, 0, bdSize, 0, NULL);
  assert(num == bdSize/bs - 1); // last block
  setupPositions(p, &num, fdArray, 1, bdSize, 1, 1, bs, bs, bs/2+1, 0, 0, 0, bdSize, 0, NULL);
  assert(num == bdSize/bs - 1); // last block
  for (size_t i = 0; i < num; i++) {
    assert(p[i].pos == i * bs);
  }
  pass();

  bs = 1<<10; // 256KiB
  for (int jump = -1000; jump < 1000; jump++) if (jump != 0) {
    num = 10000;
    setupPositions(p, &num, fdArray, 1, bdSize, 1, 1, bs, bs, bs, 0, jump, 0, bdSize, 0, NULL);
    //    fprintf(stderr,"num %zd\n", num);
    assert(num == bdSize/bs - 1); // last block
    for (size_t i = 0; i < num; i++) {
      size_t shouldbe = 0;
      if (jump > 0) {
	shouldbe = (((jump+1)*i) * bs + bdSize + bdSize) % bdSize;
      } else {
	shouldbe = (((jump)*i) * bs + bdSize + bdSize) % bdSize;
      }
      //      fprintf(stderr,"jump %d: %zd, %zd (should be %zd), pos modulo %zd\n", jump, i, p[i].pos, shouldbe, shouldbe % bdSize);
      assert(p[i].pos == shouldbe % bdSize);
    }
  }
  pass();



  free(fdArray);
  freePositions(p);
  


  yay();
  return 0;
}
