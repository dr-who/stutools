
#include <assert.h>

#include "jobType.h"
#include "positions.h"

int keepRunning = 1;
int verbose = 0;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;

  const size_t bdSize = 1024 * 1024;
  probType rorw;
  rorw.rprob=1;
  rorw.wprob=0;
  rorw.tprob=0;

  lengthsType len;
  lengthsInit(&len);
  lengthsAdd(&len, 4096, 1);

  lengthsType resetSizes;
  lengthsInit(&resetSizes);

  positionContainer pc;

  // forward
  positionContainerInit(&pc, 0);
  positionContainerSetup(&pc, 256); // setup does the alloc of positions
  positionContainerCreatePositions(&pc, 0, 1.0, 0, rorw, &len, 4096, 0, 0, bdSize, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, resetSizes);
  assert(pc.positions[0].pos == 0);
  assert(pc.positions[255].pos == 1044480);
  positionContainerDump(&pc, 10);
  positionContainerFree(&pc);

  // reverse
  positionContainerInit(&pc, 0);
  positionContainerSetup(&pc, 256);
  positionContainerCreatePositions(&pc, 0, -1.0, 0, rorw, &len, 4096, 0, 0, bdSize, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, resetSizes);
  assert(pc.positions[255].pos == 0);
  assert(pc.positions[0].pos == 1044480);
  positionContainerDump(&pc, 10);
  positionContainerFree(&pc);


  // split in 2
  positionContainerInit(&pc, 0);
  positionContainerSetup(&pc, 256);

  positionContainerCreatePositions(&pc, 0, 2, 0, rorw, &len, 4096, 0, 0, bdSize, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, resetSizes);
  // each one once
  assert(pc.positions[0].pos == 0);
  assert(pc.positions[1].pos == 1024*1024/2);
  assert(pc.positions[2].pos == 4096);
  assert(pc.positions[3].pos == 1024*1024/2 + 4096);
  assert(pc.positions[255].pos == 1044480);
  for (size_t i = 0; i <pc.sz-1; i++) {
    for (size_t j = i+1; j <pc.sz; j++) {
      assert(pc.positions[i].pos != pc.positions[j].pos);
    }
  }
  positionContainerDump(&pc, 10);
  positionContainerFree(&pc);

  // random positions
  positionContainerInit(&pc, 0);
  positionContainerSetup(&pc, 256);

  positionContainerCreatePositions(&pc, 0, 0, 0, rorw, &len, 4096, 0, 0, bdSize, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, resetSizes);
  positionContainerRandomize(&pc, 0);
  for (size_t i = 0; i <pc.sz-1; i++) {
    for (size_t j = i+1; j <pc.sz; j++) {
      //      assert(pc.positions[i].pos != pc.positions[j].pos); // not unique
    }
  }
  positionContainerDump(&pc, 10);
  positionContainerFree(&pc);

  // linear subsample

  for (size_t i = 1; i <= 256; i++) {
    positionContainerInit(&pc, 0);
    positionContainerSetup(&pc, i);
    //    pc.sz = i;
    positionContainerCreatePositions(&pc, 0, 0, 0, rorw, &len, 4096, 0, 0, bdSize, 0, 1, 0, 0, 0, 0, 0, i /*linear ss*/, 0, 0, 0, resetSizes);
    //    for (size_t i = 0; i <pc.sz-1; i++) {
    //      for (size_t j = i+1; j <pc.sz; j++) {
	//	assert(pc.positions[i].pos != pc.positions[j].pos);
    //      }
    //    }
    positionContainerDump(&pc, 10);
    positionContainerFree(&pc);
  }

  // alternate
  for (size_t i = 4; i <= 256; i+=2) { // fix 1,2,3, odd numbers
    positionContainerInit(&pc, 0);
    positionContainerSetup(&pc, i);
    //    pc.sz = i;
    positionContainerCreatePositions(&pc, 0, 1, 0, rorw, &len, 4096, 0, 0, bdSize, 0, 1, 0, 0, 0, 0, 0, i, 1 /*linear alter*/, 0, 0, resetSizes);
    for (size_t ii = 0; ii <pc.sz-1; ii++) {
      for (size_t j = i+1; j <pc.sz; j++) {
	assert(pc.positions[ii].pos != pc.positions[j].pos);
      }
    }
    positionContainerDump(&pc, 10);
    positionContainerFree(&pc);
  }

  for (size_t i = 1; i <=100000; i+=1000) 
  for (size_t s = 0; s<=1; s++) {
    // P100z
    positionContainerInit(&pc, 0);
    positionContainerSetup(&pc, i);
    //    pc.sz = i;
    positionContainerCreatePositions(&pc, 0, s, 0, rorw, &len, 4096, i/2, 0, bdSize, i+s, 1, 0, 0, 0, i /*P100*/, 0, 0, 0, 0, 0, resetSizes);
    positionContainerDump(&pc, 10);
    for (size_t ii = 0; ii <pc.sz-1; ii++) {
      for (size_t j = i+1; j <pc.sz; j++) {
	assert(pc.positions[ii].pos != pc.positions[j].pos);
      }
    }
    positionContainerFree(&pc);
  }


  /*    positionContainerInit(&pc, 0);
    positionContainerSetup(&pc, 10000000);
    createUniqueRandomPositions(&pc, &len, 0, 1024L*1024*1024*1024*1024, 4096, 0);
    positionContainerDump(&pc, 10);
    positionContainerFree(&pc);*/

    
    
    
    lengthsFree(&len);
    lengthsFree(&resetSizes);

  
  return 0;
}
