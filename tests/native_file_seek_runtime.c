#define _POSIX_C_SOURCE 200809L
#define OXID_BYTEBUF_TESTING
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/types.h>
#include <unistd.h>

static int watched_fd = -1, attempts, interrupted;
static off_t attempt_seek(int fd, off_t offset, int origin);
#define lseek attempt_seek
#include "../native/typed_bytebuf.c"
#undef lseek
static off_t attempt_seek(int fd, off_t offset, int origin) {
    if (fd == watched_fd) {
        ++attempts;
        assert(origin == SEEK_SET);
        if (interrupted) { interrupted = 0; errno = EINTR; return -1; }
    }
    return lseek(fd, offset, origin);
}
static void untouched(const oxid_binary_result *value) {
    const unsigned char *bytes = (const unsigned char *)value;
    for (size_t i = 0; i < sizeof(*value); ++i) assert(bytes[i] == 0xa5);
    assert(attempts == 0 && lseek(watched_fd, 0, SEEK_CUR) == 0);
}
int main(void) {
    const int64_t high = INT64_C(4294967296) + 123;
    int seed = open("sparse.bin", O_WRONLY | O_CREAT | O_TRUNC, 0600);
    assert(seed >= 0 && write(seed, "A", 1) == 1);
    assert(lseek(seed, (off_t)high, SEEK_SET) == high);
    assert(write(seed, "Z", 1) == 1 && close(seed) == 0);
    __oxid_bytebuf_start();
    uint64_t fuel = UINT64_MAX;
    oxid_file_open_result opened;
    assert(__oxid_file_delivery(0, NULL, (uint8_t *)"sparse.bin", 10, &fuel, &opened, 7) == 0);
    assert(opened.tag == 0);
    oxid_file file = opened.value.file;
    watched_fd = (int)execution_heap.head->initialized;
    oxid_binary_result result;
    memset(&result, 0xa5, sizeof(result));
    fuel = 6;
    assert(__oxid_file_seek_absolute(&file, high, &fuel, &result, 7) == OXID_BUFFER_FUEL);
    assert(fuel == 6); untouched(&result);
    oxid_file forged = {file.identity, 1};
    fuel = 7;
    assert(__oxid_file_seek_absolute(&forged, high, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(fuel == 7); untouched(&result);
    assert(__oxid_file_seek_absolute(&file, high, &fuel, (oxid_binary_result *)&file, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(fuel == 7 && __oxid_file_valid(&file) == 0); untouched(&result);
    assert(__oxid_file_seek_absolute(&file, high, &fuel, NULL, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(fuel == 7); untouched(&result);
    assert(__oxid_file_seek_absolute(&file, -1, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 1 && result.value == EINVAL && attempts == 0);
    assert(lseek(watched_fd, 0, SEEK_CUR) == 0);
    fuel = 7;
    assert(__oxid_file_seek_absolute(&file, high, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && attempts == 1);
    char byte;
    assert(read(watched_fd, &byte, 1) == 1 && byte == 'Z');
    interrupted = 1; fuel = 7;
    assert(__oxid_file_seek_absolute(&file, 0, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 1 && result.value == EINTR && attempts == 2);
    assert(lseek(watched_fd, 0, SEEK_CUR) == high + 1);
    fuel = 0;
    assert(__oxid_file_seek_absolute_hosted(&file, 0, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && attempts == 3);
    assert(read(watched_fd, &byte, 1) == 1 && byte == 'A');
    assert(execution_heap.ledger.allocations == 1 && execution_heap.ledger.live_bytes == 32);
    /* A real pipe establishes that ESPIPE consumes none of the queued input. */
    int descriptors[2];
    assert(pipe(descriptors) == 0 && write(descriptors[1], "Q", 1) == 1);
    assert(close(watched_fd) == 0);
    execution_heap.head->initialized = (uint32_t)descriptors[0];
    watched_fd = descriptors[0]; fuel = 7;
    assert(__oxid_file_seek_absolute(&file, 0, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 1 && result.value == ESPIPE && attempts == 4);
    assert(read(watched_fd, &byte, 1) == 1 && byte == 'Q');
    assert(close(descriptors[1]) == 0);
    __oxid_file_drop(&file);
    fuel = 7;
    assert(__oxid_file_seek_absolute(&file, 0, &fuel, &result, 7) == 0);
    assert(result.tag == 1 && result.value == EBADF && attempts == 4);
    assert(execution_heap.ledger.live_bytes == 0 && execution_heap.ledger.releases == 1);
    oxid_bytebuf buffer = {0, 0, 0}; fuel = UINT64_MAX;
    assert(oxid_bytebuf_reserve(&execution_heap, &buffer, 1, &fuel) == 0);
    forged = (oxid_file){buffer.identity, 0}; fuel = 7;
    memset(&result, 0xa5, sizeof(result));
    assert(__oxid_file_seek_absolute(&forged, 0, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(fuel == 7 && attempts == 4);
    const unsigned char *bytes = (const unsigned char *)&result;
    for (size_t i = 0; i < sizeof(result); ++i) assert(bytes[i] == 0xa5);
    __oxid_bytebuf_cleanup();
    assert(execution_heap.ledger.live_allocations == 0);
    assert(execution_heap.ledger.allocations == execution_heap.ledger.releases);
    assert(unlink("sparse.bin") == 0);
    return 0;
}
