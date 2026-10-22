/* Minimal Linux AIO ABI used only by the fault-injection test shim when the
 * system's libaio development headers are unavailable (e.g. Red Hat UBI).
 * The production C implementation still uses the system libaio.h.
 */
#ifndef STUTOOLS_TEST_AIO_ABI_H
#define STUTOOLS_TEST_AIO_ABI_H
#include <stdint.h>
#if UINTPTR_MAX != UINT64_MAX || __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error "Install the platform libaio development headers for this architecture"
#endif
typedef uintptr_t io_context_t;
enum { IO_CMD_PREAD = 0, IO_CMD_PWRITE = 1 };
struct iocb {
    uint64_t data;
    uint32_t key, aio_rw_flags;
    uint16_t aio_lio_opcode;
    int16_t aio_reqprio;
    int32_t aio_fildes;
    union {
        struct {
            void *buf;
            uint64_t nbytes;
            int64_t offset;
            uint64_t reserved;
            uint32_t flags, resfd;
        } c;
    } u;
};
struct io_event {
    uint64_t data;
    struct iocb *obj;
    uint64_t res, res2;
};
_Static_assert(sizeof(struct iocb) == 64, "Linux iocb ABI");
_Static_assert(sizeof(struct io_event) == 32, "Linux io_event ABI");
#endif
