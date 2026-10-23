#define _POSIX_C_SOURCE 200809L
#define OXID_BYTEBUF_TESTING
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

static int script_fd = -1, interrupt_read, interrupt_write, interrupt_close;
static int read_attempts, write_attempts, close_attempts, sync_error;
static ssize_t attempt_read(int fd, void *bytes, size_t length);
static ssize_t attempt_write(int fd, const void *bytes, size_t length);
static int attempt_close(int fd);
static int attempt_sync(int fd);
#define read attempt_read
#define write attempt_write
#define close attempt_close
#define fsync attempt_sync
#include "../native/typed_bytebuf.c"
#undef read
#undef write
#undef close
#undef fsync
static ssize_t attempt_read(int fd, void *bytes, size_t length) {
    if (fd == script_fd) { ++read_attempts; if (interrupt_read) { interrupt_read = 0; errno = EINTR; return -1; } }
    return read(fd, bytes, length);
}
static ssize_t attempt_write(int fd, const void *bytes, size_t length) {
    if (fd == script_fd) { ++write_attempts; if (interrupt_write) { interrupt_write = 0; errno = EINTR; return -1; } if (length > 2) length = 2; }
    return write(fd, bytes, length);
}
static int attempt_close(int fd) {
    if (fd == script_fd) {
        ++close_attempts;
        if (interrupt_close) { interrupt_close = 0; assert(close(fd) == 0); errno = EINTR; return -1; }
    }
    return close(fd);
}
static int attempt_sync(int fd) {
    if (fd == script_fd && sync_error) { errno = sync_error; return -1; }
    return fsync(fd);
}
static int32_t operation(int op, oxid_file *file, void *bytes, int32_t length, void *out) {
    uint64_t fuel = UINT64_MAX;
    return __oxid_file_delivery(op, file, bytes, length, &fuel, out, 7);
}
static void retained(void) {
    char bytes[8];
    int fd = open("keep.bin", O_RDONLY);
    assert(fd >= 0 && read(fd, bytes, sizeof(bytes)) == 8 && !memcmp(bytes, "retained", 8));
    assert(close(fd) == 0);
}
int main(void) {
    __oxid_bytebuf_start();
    int seed = open("keep.bin", O_WRONLY | O_CREAT | O_TRUNC, 0600);
    assert(seed >= 0 && write(seed, "retained", 8) == 8 && close(seed) == 0);
    const char path[] = "keep.bin";
    oxid_file_open_result opened;
    for (int boundary = 1; boundary <= 3; ++boundary) {
        execution_heap.fault_boundary = boundary;
        execution_heap.fault_skip = 0;
        assert(operation(1, NULL, (void *)path, 8, &opened) == 0 && opened.tag == 3);
        assert(execution_heap.ledger.live_allocations == 0 && execution_heap.ledger.live_bytes == 0);
        assert(execution_heap.ledger.allocations == execution_heap.ledger.releases);
        retained();
    }
    execution_heap.limit = 31;
    assert(operation(1, NULL, (void *)path, 8, &opened) == 0 && opened.tag == 2);
    retained();
    execution_heap.limit = OXID_BYTEBUF_LIMIT;
    uint64_t fuel = 31; /* 3*8+1 path work + 7 prepared return = 32 */
    assert(__oxid_file_delivery(1, NULL, (void *)path, 8, &fuel, &opened, 7) == OXID_BUFFER_FUEL);
    retained();
    const uint8_t invalid[] = "keep.bin\0suffix";
    assert(operation(1, NULL, (void *)invalid, sizeof(invalid) - 1, &opened) == 0);
    assert(opened.tag == 1 && opened.value.error == EINVAL);
    retained();
    assert(operation(0, NULL, "missing", 7, &opened) == 0 && opened.tag == 1 && opened.value.error == ENOENT);
    assert(execution_heap.ledger.live_allocations == 0 && execution_heap.ledger.peak_bytes == 32);
    execution_heap.next_identity = UINT64_MAX;
    assert(operation(1, NULL, (void *)path, 8, &opened) == OXID_BUFFER_IDENTITY);
    retained();
    execution_heap.next_identity = 1;
    fuel = 32;
    assert(__oxid_file_delivery(1, NULL, (void *)path, 8, &fuel, &opened, 7) == 0 && fuel == 0 && opened.tag == 0);
    oxid_file file = opened.value.file;
    assert(__oxid_file_valid(&file) == 0 && execution_heap.ledger.live_bytes == 32);
    script_fd = (int)execution_heap.head->initialized;
    assert(fcntl(script_fd, F_GETFD) & FD_CLOEXEC);
    oxid_binary_result result;
    interrupt_write = 1;
    assert(operation(3, &file, "abcd", 4, &result) == 0 && result.tag == 1 && result.value == EINTR && write_attempts == 1);
    assert(operation(3, &file, "abcd", 4, &result) == 0 && result.tag == 0 && result.value == 2 && write_attempts == 2);
    sync_error = ENOSPC;
    assert(operation(4, &file, NULL, 0, &result) == 0 && result.tag == 1 && result.value == ENOSPC);
    sync_error = 0;
    assert(operation(4, &file, NULL, 0, &result) == 0 && result.tag == 0);
    interrupt_close = 1;
    assert(operation(5, &file, NULL, 0, &result) == 0 && result.tag == 1 && result.value == EINTR);
    assert(file.identity == 0 && file.reserved == 0 && close_attempts == 1 && execution_heap.ledger.live_bytes == 0);
    assert(fcntl(script_fd, F_GETFD) == -1 && errno == EBADF);
    assert(operation(5, &file, NULL, 0, &result) == 0 && result.tag == 0 && close_attempts == 1);
    assert(operation(3, &file, NULL, 0, &result) == 0 && result.tag == 1 && result.value == EBADF);
    script_fd = -1;
    assert(operation(0, NULL, (void *)path, 8, &opened) == 0 && opened.tag == 0);
    file = opened.value.file;
    script_fd = (int)execution_heap.head->initialized;
    uint8_t bytes[4] = {0};
    interrupt_read = 1;
    assert(operation(2, &file, bytes, 4, &result) == 0 && result.tag == 2 && result.value == EINTR && read_attempts == 1);
    assert(operation(2, &file, bytes, 4, &result) == 0 && result.tag == 0 && result.value == 2 && !memcmp(bytes, "ab", 2));
    assert(operation(2, &file, bytes, 4, &result) == 0 && result.tag == 1);
    assert(operation(2, &file, NULL, 0, &result) == 0 && result.tag == 0 && result.value == 0);
    oxid_file forged = {file.identity, 1};
    assert(__oxid_file_valid(&forged) == OXID_BUFFER_DESCRIPTOR);
    oxid_bytebuf wrong = {file.identity, 1, 1};
    assert(__oxid_bytebuf_valid(&wrong) == OXID_BUFFER_DESCRIPTOR);
    assert(operation(2, &file, bytes, -1, &result) == OXID_BUFFER_BOUNDS);
    assert(operation(2, &file, bytes, 1, &file) == OXID_BUFFER_DESCRIPTOR);
    assert(operation(2, &file, &result, sizeof(result), &result) == OXID_BUFFER_DESCRIPTOR);
    int live_fd = script_fd;
    script_fd = -1;
    __oxid_bytebuf_cleanup();
    assert(fcntl(live_fd, F_GETFD) == -1 && errno == EBADF);
    assert(execution_heap.ledger.live_bytes == 0 && execution_heap.ledger.allocations == execution_heap.ledger.releases);
    __oxid_bytebuf_start();
    oxid_bytebuf buffer = {0, 0, 0};
    fuel = UINT64_MAX;
    assert(oxid_bytebuf_reserve(&execution_heap, &buffer, 1, &fuel) == 0);
    forged = (oxid_file){buffer.identity, 0};
    assert(__oxid_file_valid(&forged) == OXID_BUFFER_DESCRIPTOR);
    __oxid_bytebuf_cleanup();
    assert(unlink("keep.bin") == 0);
    return 0;
}
