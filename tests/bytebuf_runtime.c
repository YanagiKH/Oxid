/* Runtime-component controls only; this is not an Oxid application oracle. */
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
static ssize_t injected_read(int fd, void *bytes, size_t length);
static ssize_t injected_write(int fd, const void *bytes, size_t length);
#define read injected_read
#define write injected_write
#include "../native/experimental_bytebuf.c"
#undef read
#undef write

static int scripted, calls, step;
static uint8_t observed[32];
static size_t observed_length;
static oxid_binary_result read_attempt(uint8_t *bytes, int32_t length) {
    uint64_t fuel = 1000000;
    oxid_binary_result result;
    assert(oxid_binary_read_some(bytes, length, &fuel, &result) == 0);
    assert(fuel == 1000000 - (uint64_t)length);
    return result;
}
static oxid_binary_result write_attempt(const uint8_t *bytes, int32_t length) {
    uint64_t fuel = 1000000;
    oxid_binary_result result;
    assert(oxid_binary_write_some(bytes, length, &fuel, &result) == 0);
    assert(fuel == 1000000 - (uint64_t)length);
    return result;
}
static ssize_t injected_read(int fd, void *bytes, size_t length) {
    if (!scripted) return read(fd, bytes, length);
    ++calls;
    const uint8_t input[] = {0, 128, 255};
    switch (step++) {
        case 0: assert(length >= 3); memcpy(bytes, input, 3); return 3;
        case 1: errno = EINTR; return -1;
        case 2: assert(length >= 1); ((uint8_t *)bytes)[0] = 7; return 1;
        case 3: errno = EBADF; return -1;
        case 4: return 0;
        default: assert(0); return -1;
    }
}
static ssize_t injected_write(int fd, const void *bytes, size_t length) {
    if (!scripted) return write(fd, bytes, length);
    ++calls;
    size_t count = 0;
    switch (step++) {
        case 0: count = 2; break;
        case 1: errno = EINTR; return -1;
        case 2: count = 1; break;
        case 3: return 0;
        case 4: errno = EPIPE; return -1;
        case 5: count = 2; break;
        default: assert(0);
    }
    assert(count <= length && observed_length + count <= sizeof observed);
    memcpy(observed + observed_length, bytes, count);
    observed_length += count;
    return (ssize_t)count;
}
static void baseline(oxid_byte_heap *heap) {
    oxid_heap_cleanup(heap);
    assert(heap->head == NULL);
    assert(heap->ledger.live_bytes == 0 && heap->ledger.live_allocations == 0);
    assert(heap->ledger.allocations == heap->ledger.releases);
    oxid_heap_cleanup(heap);
}
static void equal_bytes(oxid_byte_heap *heap, oxid_bytebuf *b, const uint8_t *expected, int32_t count) {
    uint8_t *bytes;
    assert(b->length == count);
    assert(oxid_bytebuf_range(heap, b, 0, count, &bytes) == 0);
    if (count) assert(memcmp(bytes, expected, (size_t)count) == 0);
}
static void heap_tests(void) {
    for (int boundary = ALLOCATE; boundary <= COMMIT; ++boundary) {
        for (int existing = 0; existing <= 1; ++existing) {
            oxid_byte_heap heap;
            oxid_heap_init(&heap);
            oxid_bytebuf b = {0, 0, 0};
            uint64_t fuel = 1000000;
            uint8_t input[] = {0, 128, 255};
            if (existing) assert(oxid_bytebuf_extend(&heap, &b, input, 3, &fuel) == 0);
            oxid_bytebuf old = b;
            oxid_heap_ledger ledger = heap.ledger;
            heap.fault_boundary = boundary;
            assert(oxid_bytebuf_resize(&heap, &b, 100, 71, &fuel) == OXID_ALLOCATION_FAILED);
            assert(memcmp(&old, &b, sizeof b) == 0);
            assert(heap.ledger.live_bytes == ledger.live_bytes);
            assert(heap.ledger.live_allocations == ledger.live_allocations);
            equal_bytes(&heap, &b, input, existing ? 3 : 0);
            assert(oxid_bytebuf_resize(&heap, &b, 100, 71, &fuel) == 0);
            baseline(&heap);
        }
    }
    for (int repeat = 0; repeat < 100; ++repeat) {
        oxid_byte_heap heap;
        oxid_heap_init(&heap);
        oxid_bytebuf b = {0, 0, 0};
        uint64_t fuel = 1000000;
        uint8_t input[8199];
        for (size_t n = 0; n < sizeof input; ++n) input[n] = (uint8_t)(n * 73 + 19);
        for (int32_t n = 0; n < (int32_t)sizeof input; n += 1024) {
            int32_t length = (int32_t)sizeof input - n;
            if (length > 1024) length = 1024;
            assert(oxid_bytebuf_extend(&heap, &b, input + n, length, &fuel) == 0);
        }
        equal_bytes(&heap, &b, input, 8199);
        assert(heap.ledger.allocations == 5);
        assert(heap.ledger.peak_bytes == 8192 + 16384 + 64);
        assert(heap.ledger.live_bytes == 16384 + 32);
        assert(oxid_bytebuf_truncate(&heap, &b, 2) == 0);
        assert(oxid_bytebuf_resize(&heap, &b, 6, 7, &fuel) == 0);
        uint8_t expected[] = {19, 92, 7, 7, 7, 7};
        equal_bytes(&heap, &b, expected, 6);
        oxid_bytebuf stale = b;
        assert(oxid_bytebuf_release(&heap, &b) == 0);
        assert(oxid_bytebuf_push(&heap, &b, 42, &fuel) == 0);
        uint8_t *bytes;
        assert(oxid_bytebuf_range(&heap, &stale, 0, 0, &bytes) == OXID_BUFFER_DESCRIPTOR);
        assert(oxid_bytebuf_range(&heap, &b, 0, b.capacity, &bytes) == OXID_BUFFER_BOUNDS);
        baseline(&heap);
    }
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    heap.limit = 0;
    oxid_bytebuf b = {0, 0, 0};
    uint64_t fuel = 0;
    assert(oxid_bytebuf_reserve(&heap, &b, 0, &fuel) == 0);
    assert(oxid_bytebuf_extend(&heap, &b, NULL, 0, &fuel) == 0);
    assert(oxid_bytebuf_resize(&heap, &b, 0, 99, &fuel) == 0);
    assert(oxid_bytebuf_reserve(&heap, &b, -1, &fuel) == OXID_BUFFER_BOUNDS);
    assert(oxid_bytebuf_resize(&heap, &b, -1, 0, &fuel) == OXID_BUFFER_BOUNDS);
    assert(oxid_bytebuf_truncate(&heap, &b, -1) == OXID_BUFFER_BOUNDS);
    assert(oxid_bytebuf_push(&heap, &b, 1, &fuel) == OXID_MEMORY_LIMIT);
    uint8_t *bytes;
    assert(oxid_bytebuf_range(&heap, &b, 0, 0, &bytes) == 0 && bytes == NULL);
    assert(oxid_bytebuf_range(&heap, &b, -1, 0, &bytes) == OXID_BUFFER_BOUNDS);
    assert(oxid_bytebuf_range(&heap, &b, 1, 0, &bytes) == OXID_BUFFER_BOUNDS);
    assert(oxid_bytebuf_range(&heap, &b, 0, 1, &bytes) == OXID_BUFFER_BOUNDS);
    assert(heap.ledger.allocations == 0);
    heap.limit = 87;
    fuel = 8;
    assert(oxid_bytebuf_resize(&heap, &b, 8, 11, &fuel) == 0 && fuel == 0);
    assert(oxid_bytebuf_reserve(&heap, &b, INT32_MAX, &fuel) == OXID_CAPACITY_OVERFLOW);
    assert(oxid_bytebuf_push(&heap, &b, 12, &fuel) == OXID_MEMORY_LIMIT);
    heap.limit = 88;
    fuel = 8;
    assert(oxid_bytebuf_push(&heap, &b, 12, &fuel) == OXID_BUFFER_FUEL && fuel == 8);
    fuel = 9;
    assert(oxid_bytebuf_push(&heap, &b, 12, &fuel) == 0 && fuel == 0);
    assert(heap.ledger.peak_bytes == 88 && heap.ledger.live_bytes == 48);
    assert(oxid_bytebuf_growth(1073741824, 1073741825) == INT32_MAX);
    assert(oxid_bytebuf_growth(INT32_MAX, INT32_MAX) == INT32_MAX);
    baseline(&heap);
}
static void io_tests(void) {
    scripted = 1;
    uint8_t bytes[8];
    memset(bytes, 99, sizeof bytes);
    uint64_t fuel = 7;
    oxid_binary_result untouched = {99, 99};
    assert(oxid_binary_read_some(bytes, 8, &fuel, &untouched) == OXID_BUFFER_FUEL);
    assert(oxid_binary_write_some(bytes, 8, &fuel, &untouched) == OXID_BUFFER_FUEL);
    assert(oxid_binary_read_some(bytes, -1, &fuel, &untouched) == OXID_BUFFER_BOUNDS);
    assert(fuel == 7 && calls == 0 && untouched.tag == 99 && untouched.value == 99);
    for (size_t i = 0; i < sizeof bytes; ++i) assert(bytes[i] == 99);
    oxid_binary_result r = read_attempt(NULL, 0);
    assert(r.tag == 0 && r.value == 0 && calls == 0);
    r = write_attempt(NULL, 0);
    assert(r.tag == 0 && r.value == 0 && calls == 0);
    r = read_attempt(bytes, 8);
    assert(r.tag == 0 && r.value == 3);
    const uint8_t expected[] = {0, 128, 255, 99, 99, 99, 99, 99};
    assert(memcmp(bytes, expected, 8) == 0);
    r = read_attempt(bytes, 8);
    assert(r.tag == 2 && r.value == 4);
    assert(memcmp(bytes, expected, 8) == 0);
    r = read_attempt(bytes, 8);
    assert(r.tag == 0 && r.value == 1 && bytes[0] == 7);
    r = read_attempt(bytes, 8);
    assert(r.tag == 2 && r.value == 9);
    r = read_attempt(bytes, 8);
    assert(r.tag == 1 && r.value == 0 && calls == 5);
    calls = step = 0;
    const uint8_t input[] = {0, 128, 255, 7, 9};
    r = write_attempt(input, 5); assert(r.tag == 0 && r.value == 2);
    r = write_attempt(input + 2, 3); assert(r.tag == 2 && r.value == 4);
    r = write_attempt(input + 2, 3); assert(r.tag == 0 && r.value == 1);
    r = write_attempt(input + 3, 2); assert(r.tag == 0 && r.value == 0);
    r = write_attempt(input + 3, 2); assert(r.tag == 2 && r.value == 32);
    r = write_attempt(input + 3, 2); assert(r.tag == 0 && r.value == 2);
    assert(calls == 6 && observed_length == 5 && memcmp(observed, input, 5) == 0);
}
static int echo(void) {
    uint8_t bytes[1024] = {0};
    for (;;) {
        oxid_binary_result r = read_attempt(bytes, sizeof bytes);
        if (r.tag == 1) return 0;
        if (r.tag == 2) { if (r.value == EINTR) continue; return 10; }
        int32_t written = 0;
        while (written < r.value) {
            oxid_binary_result w = write_attempt(bytes + written, r.value - written);
            if (w.tag == 2) { if (w.value == EINTR) continue; return 11; }
            if (w.value == 0) return 12;
            written += w.value;
        }
    }
}
int main(int argc, char **argv) {
    if (argc == 2 && strcmp(argv[1], "echo") == 0) return echo();
    if (argc == 2 && strcmp(argv[1], "io") == 0) io_tests();
    else heap_tests();
    return 0;
}
