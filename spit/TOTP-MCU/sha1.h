#ifndef __SHA1_H
#define __SHA1_H

#include <inttypes.h>

#define HASH_LENGTH 20
#define BLOCK_LENGTH 64

void init();
void initHmac(const uint8_t* secret, uint8_t secretLength);
uint8_t* result();
uint8_t* resultHmac();
void write(uint8_t x);
void writeArray(uint8_t *buffer, uint8_t size);


#endif
