#define _POSIX_C_SOURCE 200809L
#include "native_io.h"
#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <limits.h>
#ifdef _WIN32
int native_open(native_file *f, const char *path, int writing, int create) {
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
    if (!count) return -1;
    wchar_t *wide = malloc((size_t)count * sizeof(*wide));
    if (!wide) return -1;
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, count);
    f->handle = CreateFileW(wide, GENERIC_READ | (writing ? GENERIC_WRITE : 0),
        FILE_SHARE_READ, NULL, create ? CREATE_NEW : OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    free(wide);
    return f->handle == INVALID_HANDLE_VALUE ? -1 : 0;
}
int native_size(native_file *f, uint64_t *size) {
    LARGE_INTEGER value;
    if (!GetFileSizeEx(f->handle, &value) || value.QuadPart < 0) return -1;
    *size = (uint64_t)value.QuadPart; return 0;
}
int native_resize(native_file *f, uint64_t size) {
    LARGE_INTEGER value; value.QuadPart = (LONGLONG)size;
    return SetFilePointerEx(f->handle, value, NULL, FILE_BEGIN) && SetEndOfFile(f->handle) ? 0 : -1;
}
int native_io(native_file *f, void *buf, size_t bytes, uint64_t offset, int writing) {
    LARGE_INTEGER pos; pos.QuadPart = (LONGLONG)offset;
    if (!SetFilePointerEx(f->handle, pos, NULL, FILE_BEGIN)) return -1;
    DWORD done = 0;
    BOOL ok = writing ? WriteFile(f->handle, buf, (DWORD)bytes, &done, NULL)
                      : ReadFile(f->handle, buf, (DWORD)bytes, &done, NULL);
    return ok && done == bytes ? 0 : -1;
}
int native_sync(native_file *f) { return FlushFileBuffers(f->handle) ? 0 : -1; }
void native_close(native_file *f) { CloseHandle(f->handle); }
double native_time(void) {
    LARGE_INTEGER now, frequency; QueryPerformanceCounter(&now); QueryPerformanceFrequency(&frequency);
    return (double)now.QuadPart / (double)frequency.QuadPart;
}
void native_error(const char *op) { fprintf(stderr, "%s failed (Windows error %lu)\n", op, GetLastError()); }
#else
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <time.h>
int native_open(native_file *f, const char *path, int writing, int create) {
    f->fd = open(path, (writing ? O_RDWR : O_RDONLY) | (create ? O_CREAT | O_EXCL : 0), 0600);
    if (f->fd < 0) return -1;
    struct stat st;
    if (fstat(f->fd, &st) || !S_ISREG(st.st_mode)) {
        close(f->fd); errno = EINVAL; return -1;
    }
    return 0;
}
int native_size(native_file *f, uint64_t *size) {
    struct stat st; if (fstat(f->fd, &st)) return -1;
    *size = (uint64_t)st.st_size; return 0;
}
int native_resize(native_file *f, uint64_t size) { return ftruncate(f->fd, (off_t)size); }
int native_io(native_file *f, void *buf, size_t bytes, uint64_t offset, int writing) {
    ssize_t done;
    do { done = writing ? pwrite(f->fd, buf, bytes, (off_t)offset) : pread(f->fd, buf, bytes, (off_t)offset); }
    while (done < 0 && errno == EINTR);
    if (done < 0 || (size_t)done != bytes) { if (done >= 0) errno = EIO; return -1; }
    return 0;
}
int native_sync(native_file *f) { return fsync(f->fd); }
void native_close(native_file *f) { close(f->fd); }
double native_time(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
void native_error(const char *op) { perror(op); }
#endif
