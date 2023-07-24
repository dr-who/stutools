
#include <stdlib.h>
#include <stdint.h>
#include <ctype.h>

#include "utils.h"
#include "auth.h"

#include "tpmtotp/base32.h"

#include "qr.h"


unsigned char * authGenerate(size_t *hmacKeyBytes, const size_t numberOfBits) {

  // the argument is in bits
  int bits = 80; // default with no args is 80
  if (numberOfBits) {
    bits = MAX(80, numberOfBits);
  }
  fprintf(stderr, "bits: %d\n", bits);
  
  *hmacKeyBytes = ceil (bits / 8.0); // 80 bits, 10 bytes by default

  unsigned char *hmacKey = randomGenerate(*hmacKeyBytes);
  double entropy = entropyTotalBits(hmacKey, *hmacKeyBytes, 1);
  fprintf(stderr, "Shannon entropy: %.0lf bits\n", entropy);

  return hmacKey;
}

char * authString(const unsigned char *hmacKey, const size_t hmacKeyBytes) {
  if (hmacKey == NULL) return NULL;

  fprintf(stderr, "HMAC size: %zd bytes (%zd bits)\n", hmacKeyBytes, hmacKeyBytes * 8);
  uint8_t *p = (uint8_t*)hmacKey;
  fprintf(stderr, "RAW random: ");
  for (size_t i = 0; i < hmacKeyBytes; i++) {
    fprintf(stderr, "%02x ", *p);
    p++;
  }
  fprintf(stderr, "\n");
  const size_t codedSize = (int)(ceil(hmacKeyBytes * 8.0 / 5));
  fprintf(stderr, "base32 size: %zd bytes\n", codedSize);
  unsigned char coded[1+codedSize];
  memset(coded, 0, 1+codedSize);
  base32_encode(hmacKey, hmacKeyBytes, coded);
  coded[codedSize] = 0;

  fprintf(stderr, "key: '%s'\n", coded);

  return strdup((char*)coded);
}

unsigned char * authSet(size_t *hmacKeyBytes, const char *encoded32) {
  unsigned char *hmacKey = NULL;
  if (encoded32) {
    char *key = strdup(encoded32);
    for (size_t i = 0; i < strlen(key); i++) {
      if (islower(key[i])) key[i] = toupper(key[i]);
    }
    
    hmacKey = calloc(strlen(key), 1);
    *hmacKeyBytes = base32_decode((unsigned char*)key, hmacKey);
    //    hmacKeyBytes = strlen((char*)hmacKey);
    if (*hmacKeyBytes < 10) {
      fprintf(stderr,"*error* too few bytes '%s'\n", encoded32);
      exit(-1);
      //      cmd_authClear();
    }
    fprintf(stderr,"authset: '%s'\n", encoded32);
    free(key);
  }
  return hmacKey;
}



  
void authPrintQR(FILE *fp, const char *coded32, const char *str1, const char *str2) {

  if(coded32) {
    const size_t sizeBits = strlen(coded32) * 5;
    
    char s[1024];
    sprintf(s, "otpauth://totp/%s@%s-%zd-bits?secret=%s&issuer=stush", str1, str2, sizeBits, coded32);
    qrGenerate(fp, s);
  } 
}

void authPrint(FILE *fp, const unsigned char *hmacKey, size_t hmacKeyBytes) {
  if (hmacKey) {
    char *str = authString(hmacKey, hmacKeyBytes);
    fprintf(fp, "%s\n", str);
    free(str);
  }
}

  
