/* Private Linux x86_64 experimental byte-buffer runtime.
 * The registry and usable bytes occupy one fallible allocation. No separately
 * allocated metadata, implicit retries, or user-visible pointer/handle exists.
 */
#include "experimental_bytebuf.h"
#include <errno.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct oxid_allocation {
    oxid_allocation *next, *previous;
    uint64_t identity;
    uint32_t capacity, initialized;
};
_Static_assert(sizeof(oxid_allocation) == 32, "Linux x86_64 registry header");
_Static_assert(sizeof(oxid_bytebuf) == 16, "private descriptor");
_Static_assert(_Alignof(oxid_bytebuf) == 8, "private descriptor alignment");
#ifdef OXID_BYTEBUF_TESTING
_Static_assert(sizeof(oxid_byte_heap) == 72, "test execution context");
#else
_Static_assert(sizeof(oxid_byte_heap) == 64, "execution context");
#endif

enum boundary { ALLOCATE = 1, INITIALIZE = 2, COMMIT = 3 };
static int fail(oxid_byte_heap *heap, enum boundary boundary) {
#ifdef OXID_BYTEBUF_TESTING
    if (heap->fault_boundary == (int32_t)boundary) {
        if (heap->fault_skip == 0) { heap->fault_boundary = 0; return 1; }
        --heap->fault_skip;
    }
#else
    (void)heap;
    (void)boundary;
#endif
    return 0;
}
void oxid_heap_init(oxid_byte_heap *heap) {
    memset(heap, 0, sizeof *heap);
    heap->next_identity = 1;
    heap->limit = OXID_BYTEBUF_LIMIT;
}
static uint8_t *data(oxid_allocation *node) { return (uint8_t *)(node + 1); }
static int32_t locate(oxid_byte_heap *heap, const oxid_bytebuf *value,
    oxid_allocation **out) {
    *out = NULL;
    if (value->identity == 0)
        return value->length == 0 && value->capacity == 0 ? 0 : OXID_BUFFER_DESCRIPTOR;
    if (value->length < 0 || value->capacity <= 0 || value->length > value->capacity)
        return OXID_BUFFER_DESCRIPTOR;
    for (oxid_allocation *node = heap->head; node; node = node->next) {
        if (node->identity == value->identity) {
            if (node->capacity != (uint32_t)value->capacity ||
                node->initialized != (uint32_t)value->length) return OXID_BUFFER_DESCRIPTOR;
            *out = node;
            return 0;
        }
    }
    return OXID_BUFFER_DESCRIPTOR;
}
static void unlink_node(oxid_byte_heap *heap, oxid_allocation *node) {
    if (node->previous) node->previous->next = node->next;
    else heap->head = node->next;
    if (node->next) node->next->previous = node->previous;
    uint32_t capacity = node->capacity;
    if (capacity == UINT32_MAX) {
        if (node->initialized != UINT32_MAX) (void)close((int)node->initialized);
        capacity = 0;
    }
    heap->ledger.live_bytes -= (uint64_t)capacity + OXID_BYTEBUF_HEADER;
    --heap->ledger.live_allocations;
    ++heap->ledger.releases;
    free(node);
}
int32_t oxid_bytebuf_validate(oxid_byte_heap *heap, const oxid_bytebuf *value) {
    oxid_allocation *node;
    return locate(heap, value, &node);
}
void oxid_heap_cleanup(oxid_byte_heap *heap) {
    while (heap->head) unlink_node(heap, heap->head);
}
int32_t oxid_bytebuf_release(oxid_byte_heap *heap, oxid_bytebuf *value) {
    oxid_allocation *node;
    int32_t status = locate(heap, value, &node);
    if (status) return status;
    if (node) unlink_node(heap, node);
    *value = (oxid_bytebuf){0, 0, 0};
    return 0;
}
int32_t oxid_bytebuf_range(oxid_byte_heap *heap, const oxid_bytebuf *value,
    int32_t start, int32_t end, uint8_t **bytes) {
    oxid_allocation *node;
    *bytes = NULL;
    int32_t status = locate(heap, value, &node);
    if (status) return status;
    if (start < 0 || end < start || end > value->length) return OXID_BUFFER_BOUNDS;
    *bytes = node ? data(node) + start : NULL;
    return 0;
}
int32_t oxid_bytebuf_growth(int32_t current, int32_t required) {
    if (current < 0 || required < 0) return OXID_BUFFER_BOUNDS;
    if (required <= current) return current;
    int32_t doubled = current > INT32_MAX / 2 ? INT32_MAX : current * 2;
    if (doubled < 8) doubled = 8;
    return required > doubled ? required : doubled;
}
enum mutation { RESERVE, EXTEND, RESIZE };
static int32_t change(oxid_byte_heap *heap, oxid_bytebuf *value, int32_t required,
    enum mutation kind, const uint8_t *source, uint8_t fill, uint64_t *fuel) {
    oxid_allocation *old;
    int32_t status = locate(heap, value, &old);
    if (status) return status;
    int32_t capacity = oxid_bytebuf_growth(value->capacity, required);
    if (capacity < 0) return capacity;
    int grows = capacity != value->capacity;
    int32_t length = kind == RESERVE ? value->length : required;
    uint64_t writes = kind != RESERVE && length > value->length ?
        (uint64_t)(length - value->length) : 0;
    if (!grows && length == value->length) return OXID_GROW_OK;
    uint64_t charge = (uint64_t)capacity + OXID_BYTEBUF_HEADER;
    if (grows && (charge > heap->limit || heap->ledger.live_bytes > heap->limit - charge))
        return OXID_MEMORY_LIMIT;
    if (grows && heap->next_identity == UINT64_MAX) return OXID_BUFFER_IDENTITY;
    uint64_t work = writes + (grows ? (uint64_t)value->length : 0);
    if (*fuel < work) return OXID_BUFFER_FUEL;
    *fuel -= work;
    oxid_allocation *target = old;
    if (grows) {
        if (fail(heap, ALLOCATE)) return OXID_ALLOCATION_FAILED;
        target = malloc(sizeof *target + (size_t)capacity);
        if (!target) return OXID_ALLOCATION_FAILED;
        /* No fallible operation can occur between malloc and registry linking. */
        *target = (oxid_allocation){heap->head, NULL, heap->next_identity++, (uint32_t)capacity, 0};
        if (heap->head) heap->head->previous = target;
        heap->head = target;
        heap->ledger.live_bytes += charge;
        ++heap->ledger.live_allocations;
        ++heap->ledger.allocations;
        if (heap->ledger.live_bytes > heap->ledger.peak_bytes)
            heap->ledger.peak_bytes = heap->ledger.live_bytes;
        if (fail(heap, INITIALIZE)) { unlink_node(heap, target); return OXID_ALLOCATION_FAILED; }
    }
    if (fail(heap, COMMIT)) {
        if (grows) unlink_node(heap, target);
        return OXID_ALLOCATION_FAILED;
    }
    /* All remaining operations are infallible. Only initialized bytes move. */
    if (target) {
        if (grows && old && value->length) memcpy(data(target), data(old), (size_t)value->length);
        if (writes) {
            if (kind == EXTEND) memcpy(data(target) + value->length, source, (size_t)writes);
            else memset(data(target) + value->length, fill, (size_t)writes);
        }
        target->initialized = (uint32_t)length;
        value->identity = target->identity;
    }
    value->length = length;
    value->capacity = capacity;
    if (grows && old) unlink_node(heap, old);
    return OXID_GROW_OK;
}
int32_t oxid_bytebuf_reserve(oxid_byte_heap *heap, oxid_bytebuf *value,
    int32_t additional, uint64_t *fuel) {
    oxid_allocation *node;
    int32_t status = locate(heap, value, &node);
    if (status) return status;
    if (additional < 0) return OXID_BUFFER_BOUNDS;
    if (additional > INT32_MAX - value->length) return OXID_CAPACITY_OVERFLOW;
    return change(heap, value, value->length + additional, RESERVE, NULL, 0, fuel);
}
int32_t oxid_bytebuf_extend(oxid_byte_heap *heap, oxid_bytebuf *value,
    const uint8_t *source, int32_t length, uint64_t *fuel) {
    oxid_allocation *node;
    int32_t status = locate(heap, value, &node);
    if (status) return status;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if (length > INT32_MAX - value->length) return OXID_CAPACITY_OVERFLOW;
    /* The sealed caller must prove source's initialized extent and non-aliasing
     * loan before this private pointer boundary. Null is valid only for empty. */
    if (length && !source) return OXID_BUFFER_DESCRIPTOR;
    return change(heap, value, value->length + length, EXTEND, source, 0, fuel);
}
int32_t oxid_bytebuf_push(oxid_byte_heap *heap, oxid_bytebuf *value,
    uint8_t byte, uint64_t *fuel) {
    return oxid_bytebuf_extend(heap, value, &byte, 1, fuel);
}
int32_t oxid_bytebuf_resize(oxid_byte_heap *heap, oxid_bytebuf *value,
    int32_t length, uint8_t fill, uint64_t *fuel) {
    oxid_allocation *node;
    int32_t status = locate(heap, value, &node);
    if (status) return status;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    return change(heap, value, length, RESIZE, NULL, fill, fuel);
}
int32_t oxid_bytebuf_truncate(oxid_byte_heap *heap, oxid_bytebuf *value, int32_t length) {
    oxid_allocation *node;
    int32_t status = locate(heap, value, &node);
    if (status) return status;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if (length < value->length) value->length = length;
    if (node) node->initialized = (uint32_t)value->length;
    return 0;
}

static oxid_binary_result read_once(uint8_t *bytes, int32_t length) {
    if (!length) return (oxid_binary_result){0, 0};
    ssize_t done = read(STDIN_FILENO, bytes, (size_t)length);
    if (done < 0) return (oxid_binary_result){2, errno};
    if (!done) return (oxid_binary_result){1, 0};
    if (done > length) return (oxid_binary_result){2, EIO};
    return (oxid_binary_result){0, (int32_t)done};
}
static oxid_binary_result write_once(const uint8_t *bytes, int32_t length) {
    if (!length) return (oxid_binary_result){0, 0};
    ssize_t done = write(STDOUT_FILENO, bytes, (size_t)length);
    if (done < 0) return (oxid_binary_result){2, errno};
    if (done > length) return (oxid_binary_result){2, EIO};
    return (oxid_binary_result){0, (int32_t)done};
}
static int32_t admit_attempt(const uint8_t *bytes, int32_t length, uint64_t *fuel,
    oxid_binary_result *result) {
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if ((length && !bytes) || !fuel || !result) return OXID_BUFFER_DESCRIPTOR;
    if (*fuel < (uint64_t)length) return OXID_BUFFER_FUEL;
    *fuel -= (uint64_t)length;
    return 0;
}
int32_t oxid_binary_read_some(uint8_t *bytes, int32_t length, uint64_t *fuel,
    oxid_binary_result *result) {
    int32_t status = admit_attempt(bytes, length, fuel, result);
    if (status) return status;
    *result = read_once(bytes, length);
    return 0;
}
int32_t oxid_binary_write_some(const uint8_t *bytes, int32_t length, uint64_t *fuel,
    oxid_binary_result *result) {
    int32_t status = admit_attempt(bytes, length, fuel, result);
    if (status) return status;
    *result = write_once(bytes, length);
    return 0;
}
