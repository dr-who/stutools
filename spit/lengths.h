#ifndef _LENGTHS_H
#define _LENGTHS_H

typedef struct {
  size_t size;
  size_t *len;
  size_t *freq;
  size_t lastpos;
  size_t sum;
  size_t min;
  size_t max;
} lengthsType;

void lengthsInit(lengthsType *l);
void lengthsFree(lengthsType *l);

void lengthsAdd(lengthsType *l, const size_t len, size_t freq);
size_t lengthsSize(const lengthsType *l);
size_t lengthsGet(const lengthsType *l, unsigned int *seed);
size_t lengthsMin(const lengthsType *l);
size_t lengthsMax(const lengthsType *l);

size_t lengthsParse(lengthsType *l, const char *str);


void lengthsSetupLowHighAlignSeq(lengthsType *l, size_t min, size_t max, size_t align);
void lengthsSetupLowHighAlignPower(lengthsType *l, size_t min, size_t max, size_t align);

void lengthsDump(const lengthsType *l, const char *prefix);

#endif
