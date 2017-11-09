#ifndef _AIO_OPERATIONS
#define _AIO_OPERATIONS


long readNonBlocking(const char *path, const size_t BLKSIZE, const size_t maxBytes, const float secTimeout, int quiet);
long writeNonBlocking(const char *path, const size_t BLKSIZE, const size_t maxBytes, const float secTimeout, int quiet);


#endif
