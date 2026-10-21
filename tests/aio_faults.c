#define _GNU_SOURCE
#include <dlfcn.h>
#include <libaio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void *pending_buffer, *pending_cb;
static int destroyed;
static const char *mode(void) {
    const char *m = getenv("SPIT_AIO_FAULT");
    return m ? m : "";
}
int io_submit(io_context_t ctx, long nr, struct iocb **cbs) {
    int (*real_submit)(io_context_t, long, struct iocb **) = dlsym(RTLD_NEXT, "io_submit");
    int ret = real_submit(ctx, nr, cbs);
    if (ret > 0) {
        pending_buffer = cbs[0]->u.c.buf;
        pending_cb = cbs[0];
        destroyed = 0;
    }
    return ret;
}
int io_getevents(io_context_t ctx, long min, long max, struct io_event *events, struct timespec *timeout) {
    if (!strcmp(mode(), "delay") || !strcmp(mode(), "destroy_fail")) return 0;
    int (*real_get)(io_context_t, long, long, struct io_event *, struct timespec *) = dlsym(RTLD_NEXT, "io_getevents");
    int ret = real_get(ctx, min, max, events, timeout);
    for (int i = 0; i < ret; i++) {
        if (!strcmp(mode(), "short")) events[i].res = events[i].obj->u.c.nbytes / 2;
        if (!strcmp(mode(), "error")) events[i].res = (unsigned long)-5;
    }
    return ret;
}
int io_destroy(io_context_t ctx) {
    if (!strcmp(mode(), "destroy_fail")) return -5;
    int (*real_destroy)(io_context_t) = dlsym(RTLD_NEXT, "io_destroy");
    int ret = real_destroy(ctx);
    if (!ret) destroyed = 1;
    return ret;
}
void free(void *ptr) {
    /* glibc entry point avoids recursive dlsym allocation during startup. */
    extern void __libc_free(void *);
    if (ptr && (!strcmp(mode(), "delay") || !strcmp(mode(), "destroy_fail")) &&
        !destroyed && (ptr == pending_buffer || ptr == pending_cb)) {
        const char msg[] = "FREED_PENDING_IO\n";
        write(2, msg, sizeof(msg)-1);
        _exit(90);
    }
    __libc_free(ptr);
}
int usleep(useconds_t usec) {
    if (!strcmp(mode(), "delay") || !strcmp(mode(), "destroy_fail")) return 0;
    int (*real_sleep)(useconds_t) = dlsym(RTLD_NEXT, "usleep");
    return real_sleep(usec);
}
