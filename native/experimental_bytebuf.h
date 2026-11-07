/* Private experimental runtime called only by verified generated adapters. */
#ifndef OXID_EXPERIMENTAL_BYTEBUF_H
#define OXID_EXPERIMENTAL_BYTEBUF_H
#include <stddef.h>
#include <stdint.h>

#ifndef OXID_BYTEBUF_LIMIT
#define OXID_BYTEBUF_LIMIT UINT64_C(16777216)
#endif
#define OXID_BYTEBUF_HEADER UINT64_C(32)
enum oxid_grow_status {
    OXID_GROW_OK = 0, OXID_CAPACITY_OVERFLOW = 1,
    OXID_ALLOCATION_FAILED = 2, OXID_MEMORY_LIMIT = 3,
    OXID_BUFFER_BOUNDS = -1, OXID_BUFFER_FUEL = -2,
    OXID_BUFFER_DESCRIPTOR = -3, OXID_BUFFER_IDENTITY = -4
};
typedef struct { uint64_t identity; int32_t length, capacity; } oxid_bytebuf;
typedef struct oxid_allocation oxid_allocation;
typedef struct {
    uint64_t live_bytes, live_allocations, peak_bytes, allocations, releases;
} oxid_heap_ledger;
typedef struct {
    oxid_allocation *head;
    uint64_t next_identity, limit;
    oxid_heap_ledger ledger;
#ifdef OXID_BYTEBUF_TESTING
    int32_t fault_boundary;
    uint32_t fault_skip;
#endif
} oxid_byte_heap;

void oxid_heap_init(oxid_byte_heap *heap);
void oxid_heap_cleanup(oxid_byte_heap *heap);
int32_t oxid_bytebuf_validate(oxid_byte_heap *heap, const oxid_bytebuf *value);
int32_t oxid_bytebuf_release(oxid_byte_heap *heap, oxid_bytebuf *value);
int32_t oxid_bytebuf_range(oxid_byte_heap *heap, const oxid_bytebuf *value,
    int32_t start, int32_t end, uint8_t **bytes);
int32_t oxid_bytebuf_reserve(oxid_byte_heap *heap, oxid_bytebuf *value,
    int32_t additional, uint64_t *fuel);
int32_t oxid_bytebuf_push(oxid_byte_heap *heap, oxid_bytebuf *value,
    uint8_t byte, uint64_t *fuel);
int32_t oxid_bytebuf_extend(oxid_byte_heap *heap, oxid_bytebuf *value,
    const uint8_t *source, int32_t length, uint64_t *fuel);
int32_t oxid_bytebuf_resize(oxid_byte_heap *heap, oxid_bytebuf *value,
    int32_t length, uint8_t fill, uint64_t *fuel);
int32_t oxid_bytebuf_truncate(oxid_byte_heap *heap, oxid_bytebuf *value, int32_t length);
int32_t oxid_bytebuf_growth(int32_t current, int32_t required);

/* Private attempt result: tag 0 = progress, 1 = EOF, 2 = errno.
 * Writes never produce tag 1. A zero-progress write is returned as progress(0).
 * The caller must admit the result slot and the loan before this boundary.
 * These routines validate the extent and prepay byte work before the syscall;
 * returning the result cannot fail. A negative return is a fatal runtime error.
 */
typedef struct { int32_t tag, value; } oxid_binary_result;
int32_t oxid_binary_read_some(uint8_t *bytes, int32_t length, uint64_t *fuel,
    oxid_binary_result *result);
int32_t oxid_binary_write_some(const uint8_t *bytes, int32_t length, uint64_t *fuel,
    oxid_binary_result *result);
#endif
