#define _GNU_SOURCE
#include <dlfcn.h>
#if __has_include(<libaio.h>)
#include <libaio.h>
#else
#include "aio_abi.h"
#endif
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static void *pending_buffer, *pending_cb;
static int destroyed;
static long queued, max_queued, max_submit, mixed_submits;
static long total_submitted, get_calls, refilled_early;
static struct iocb *active[4096];
static const char *mode(void) {
    const char *m = getenv("SPIT_AIO_FAULT");
    return m ? m : "";
}
int io_setup(int maxevents, io_context_t *ctx) {
    int (*real_setup)(int, io_context_t *) = dlsym(RTLD_NEXT, "io_setup");
    static int setups;
    if (!strcmp(mode(), "setup_once") && __sync_fetch_and_add(&setups, 1) == 1) return -12;
    return real_setup(maxevents, ctx);
}
int io_submit(io_context_t ctx, long nr, struct iocb **cbs) {
    int (*real_submit)(io_context_t, long, struct iocb **) = dlsym(RTLD_NEXT, "io_submit");
    static int failed;
    if (!strcmp(mode(), "submit_once") && __sync_fetch_and_add(&failed, 1) == 0) return -5;
    if (!strcmp(mode(), "depth") || !strcmp(mode(), "refill")) {
        for (long i = 0; i < nr; i++) {
            for (size_t j = 0; j < 4096; j++) {
                if (active[j] == cbs[i]) {
                    const char msg[] = "REUSED_PENDING_IO\n";
                    write(2, msg, sizeof(msg)-1);
                    return -5;
                }
            }
        }
    }
    int ret = real_submit(ctx, nr, cbs);
    if (ret > 0) {
        if (!strcmp(mode(), "depth") || !strcmp(mode(), "refill")) {
            total_submitted += ret;
            for (int i = 0; i < ret; i++) {
                for (size_t j = 0; j < 4096; j++) {
                    if (!active[j]) { active[j] = cbs[i]; break; }
                }
            }
            queued += ret;
            if (queued > max_queued) max_queued = queued;
            if (ret > max_submit) max_submit = ret;
            for (int i = 1; i < ret; i++) {
                if (cbs[i]->aio_lio_opcode != cbs[0]->aio_lio_opcode) {
                    mixed_submits++;
                    break;
                }
            }
        }
        if (!strcmp(mode(), "delay") || !strcmp(mode(), "destroy_fail")) {
            pending_buffer = cbs[0]->u.c.buf;
            pending_cb = cbs[0];
            destroyed = 0;
        }
    }
    return ret;
}
int io_getevents(io_context_t ctx, long min, long max, struct io_event *events, struct timespec *timeout) {
    if (!strcmp(mode(), "delay") || !strcmp(mode(), "destroy_fail")) return 0;
    int (*real_get)(io_context_t, long, long, struct io_event *, struct timespec *) = dlvsym(RTLD_NEXT, "io_getevents", "LIBAIO_0.4");
    /* Older glibc dlsym(RTLD_NEXT) selects LIBAIO_0.1, whose ABI has
       four arguments rather than five. Select the current ABI explicitly. */
    if (!real_get) {
        const char msg[] = "Cannot resolve io_getevents@LIBAIO_0.4\n";
        write(2, msg, sizeof(msg)-1);
        _exit(91);
    }
    if (!strcmp(mode(), "refill")) {
        max = 1;
        if (++get_calls == 2) refilled_early = total_submitted > 32;
    }
    int ret = real_get(ctx, min, max, events, timeout);
    if ((!strcmp(mode(), "depth") || !strcmp(mode(), "refill")) && ret > 0) queued -= ret;
    if (!strcmp(mode(), "depth") || !strcmp(mode(), "refill")) {
        for (int i = 0; i < ret; i++) {
            for (size_t j = 0; j < 4096; j++) {
                if (active[j] == events[i].obj) { active[j] = NULL; break; }
            }
        }
    }
    static int reads;
    for (int i = 0; i < ret; i++) {
        if (events[i].obj->aio_lio_opcode == IO_CMD_PREAD && (long)events[i].res > 0) {
            int read_count = !strcmp(mode(), "corrupt_last") ? __sync_add_and_fetch(&reads, 1) : 0;
            if (!strcmp(mode(), "corrupt") ||
                (!strcmp(mode(), "corrupt_last") && read_count == 2)) {
                ((char *)events[i].obj->u.c.buf)[0] ^= 1;
            }
        }
        if (!strcmp(mode(), "short")) events[i].res = events[i].obj->u.c.nbytes / 2;
        if (!strcmp(mode(), "error")) events[i].res = (unsigned long)-5;
    }
    return ret;
}
int io_destroy(io_context_t ctx) {
    if (!strcmp(mode(), "destroy_fail")) return -5;
    int (*real_destroy)(io_context_t) = dlsym(RTLD_NEXT, "io_destroy");
    int ret = real_destroy(ctx);
    if (!strcmp(mode(), "depth") || !strcmp(mode(), "refill")) {
        char msg[160];
        int len = snprintf(msg, sizeof(msg), "MAX_PENDING=%ld MAX_SUBMIT=%ld MIXED_SUBMITS=%ld REFILLED_EARLY=%ld\n", max_queued, max_submit, mixed_submits, refilled_early);
        if (len > 0) write(2, msg, (size_t)len);
    }
    if (!ret && (!strcmp(mode(), "delay") || !strcmp(mode(), "destroy_fail"))) destroyed = 1;
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
