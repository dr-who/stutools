#ifndef _AUTH_H
#define _AUTH_H

// TOTP functions

// returns an array of randomness/the key
unsigned char * authGenerate(size_t *hmacKeyBytes, const size_t numberOfBits);

// encodes the key to an encoded32 string
char * authString(const unsigned char *hmacKey, const size_t hmacKeyBytes);

// decodes the encoded32 string, returns the random values
unsigned char * authSet(size_t *hmacKeyBytes, const char *encoded32);

// generates a TOTP QR code
void authPrintQR(FILE *fp, const char *coded32, const char *str1, const char *str2);

// generates the string, prints it and frees it
void authPrint(FILE *fp, const unsigned char *hmacKey, size_t hmacKeyBytes);

#endif


