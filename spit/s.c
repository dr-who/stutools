#include <stdio.h>
#include <stdlib.h>
#include <openssl/sha.h>

typedef unsigned char byte;

int main(int argc, char *argv[]) {
    const int DataLen = 3;
    SHA_CTX shactx;
    byte digest[1024];



		 
		 
    int i;

    byte* testdata = (byte *)malloc(DataLen);
    for (i=0; i<DataLen; i++) testdata[i] = 0;
    testdata[0] = 'a';
    testdata[1] = 'b';
    testdata[2] = 'c';

    EVP_Q_digest(testdata, 3, digest, NULL, NULL, "SHA1", NULL);

    for (i=0; i<SHA_DIGEST_LENGTH; i++)
	printf("%02x", digest[i]);
    putchar('\n');

    return 0;
}
