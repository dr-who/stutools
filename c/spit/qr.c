#include <stdio.h>
#include <stdlib.h>

#include "qr/qrcodegen.h"

static void printQr(FILE *fp, const uint8_t qrcode[]) {
  int size = qrcodegen_getSize(qrcode);
  int border = 4;
  for (int y = -border; y < size + border; y++) {
    for (int x = -border; x < size + border; x++) {
      fputs((qrcodegen_getModule(qrcode, x, y) ? "\x1b[1;40m  \x1b[0;m" : "\x1b[1;107m  \x1b[0;m"), fp);
    }
    //		fputs("\x1b[0;m", stdout);
    fputs("\n", fp);
  }
  fflush(fp);
}

void qrGenerate(FILE *fp, const char *text) {
  enum qrcodegen_Ecc errCorLvl = qrcodegen_Ecc_LOW;  // Error correction level
  
  // Make and print the QR Code symbol
  uint8_t qrcode[qrcodegen_BUFFER_LEN_MAX];
  uint8_t tempBuffer[qrcodegen_BUFFER_LEN_MAX];
  bool ok = qrcodegen_encodeText(text, tempBuffer, qrcode, errCorLvl,
				 qrcodegen_VERSION_MIN, qrcodegen_VERSION_MAX, qrcodegen_Mask_AUTO, true);
  if (ok) {
    printQr(fp, qrcode);
  }
}

