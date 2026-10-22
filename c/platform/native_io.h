#ifndef STUTOOLS_NATIVE_IO_H
#define STUTOOLS_NATIVE_IO_H
#include <stdint.h>
#include <stddef.h>
#ifdef _WIN32
#include <windows.h>
typedef struct { HANDLE handle; } native_file;
#else
typedef struct { int fd; } native_file;
#endif
int native_open(native_file *f, const char *path, int write_access, int create);
int native_size(native_file *f, uint64_t *size);
int native_resize(native_file *f, uint64_t size);
int native_io(native_file *f, void *buffer, size_t bytes, uint64_t offset, int writing);
int native_sync(native_file *f);
void native_close(native_file *f);
double native_time(void);
void native_error(const char *operation);
#endif
