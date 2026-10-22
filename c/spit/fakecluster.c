#include <stdio.h>
#include <malloc.h>

#include "fakecluster.h"

fakeclusterType *fakeclusterInit(int size) {
  fakeclusterType *c = calloc(size, sizeof(fakeclusterType));

  for (int i = 0; i < size; i++ ){
    c[i].id = i;
    c[i].time = 0;
    c[i].RAMGB = 4+(i % 8);
    c[i].numGPUs = i%3;
    c[i].numDevices = 20+3*i;
    c[i].bdSize = (i%2) ? 18000 : 12000;
  }

  return c;
}

void dumpNode(fakeclusterType *c) {
  // cpu
  // gpu
  // device
  // name, size, type, serial, acores, bcores 
  // d;23 d#0|name:/dev/sda|size:18000|type:SSD|serial:912812| d#1|name...
  
  printf("%d %lf %d %d %d %d\n", c->id, c->time, c->RAMGB, c->numGPUs, c->numDevices, c->bdSize);
  
}


void fakeclusterDump(fakeclusterType *c, int size) {
  for (int i = 0; i < size; i++) {
    dumpNode(&c[i]);
  }
}


int main(void) {

  int size = 100;
  fakeclusterType *cluster = fakeclusterInit(size);

  fakeclusterDump(cluster, size);

  free(cluster);
  
  return 0;
}

