/* Runtime component controls, independent of source lowering and LLVM emission. */
#define _POSIX_C_SOURCE 200809L
#define OXID_BYTEBUF_TESTING
#define OXID_OWNED_VECTOR_RUNTIME
#define OXID_OWNED_VECTOR_SCHEMAS
#define OXID_RECORD_VECTOR_SCHEMAS
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static int force_null;
static void *owned_test_allocate(size_t alignment, size_t bytes) {
    return force_null ? NULL : aligned_alloc(alignment, bytes);
}
#define OXID_OWNED_VECTOR_ALIGNED_ALLOC owned_test_allocate
#include "../native/experimental_bytebuf.c"
#include "../native/experimental_owned_vector.c"

static const oxid_vector_record_leaf copy_leaves[] = {{0, 0, OXID_VECTOR_U32}};
#define COPY_RECORD_TAG (OXID_VECTOR_RECORD_BIT | 2u)
const uint32_t __oxid_record_vector_schema_count = 3;
const oxid_vector_record_schema __oxid_record_vector_schemas[] = {
    {0, 0, 0, 0, NULL}, {0, 0, 0, 0, NULL},
    {COPY_RECORD_TAG, 4, 8, 1, copy_leaves}
};
#ifndef OXID_TEST_GENERATED_OWNED_SCHEMAS
const oxid_owned_type __oxid_owned_types[] = {
    {OXID_OWNED_SCALAR, OXID_VECTOR_BOOL, 0, 0, 1},
    {OXID_OWNED_BYTEBUF, 0, 0, 0, 16},
    {OXID_OWNED_SCALAR, OXID_VECTOR_I32, 0, 0, 4},
    {OXID_OWNED_RECORD, 0, 0, 3, 32}
};
const oxid_owned_field __oxid_owned_fields[] = {{0, 0}, {1, 8}, {2, 24}};
const oxid_owned_field __oxid_owned_variants[] = {{UINT32_MAX, 0}};
const oxid_owned_record_schema __oxid_owned_records[] = {
    {0, 3, 32, 7, 1}, {1, 3, 32, 7, 1}, {2, UINT32_MAX, 32, 1, 0}
};
const uint32_t __oxid_owned_type_count = 4, __oxid_owned_field_count = 3;
const uint32_t __oxid_owned_variant_count = 0, __oxid_owned_record_count = 3;
#endif

static uint64_t enough(void) { return UINT64_C(1000000); }
static oxid_bytebuf incoming(oxid_byte_heap *heap, uint8_t record[32], uint8_t value) {
    oxid_bytebuf child = {0, 0, 0};
    uint64_t fuel = enough();
    assert(oxid_bytebuf_push(heap, &child, value, &fuel) == OXID_GROW_OK);
    memset(record, 0xa5, 32); record[0] = 1;
    memcpy(record + 8, &child, 16);
    record[24] = value; record[25] = 0; record[26] = 0; record[27] = 0;
    return child;
}
static void reserve(oxid_byte_heap *heap, oxid_owned_vector *value, int32_t additional) {
    uint64_t fuel = enough();
    assert(oxid_owned_vector_reserve(heap, value, 0, additional, &fuel) == OXID_GROW_OK);
}
static void canonical(const uint8_t *record, const oxid_bytebuf *child, uint8_t value) {
    uint8_t expected[32] = {0}; expected[0] = 1;
    memcpy(expected + 8, child, 16); expected[24] = value;
    assert(!memcmp(record, expected, 32));
}
static void move_round_trip_and_literal_work(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector value, moved;
    assert(oxid_owned_vector_new(0, &value) == 0);
    assert(oxid_owned_vector_new(2, &moved) == OXID_BUFFER_DESCRIPTOR);
    uint8_t source[32], result[32];
    oxid_bytebuf child = incoming(&heap, source, 73);
    uint64_t fuel = 391;
    assert(oxid_owned_vector_reserve(&heap, &value, 0, 1, &fuel) == OXID_GROW_OK && fuel == 0);
    assert(value.length == 0 && value.capacity == 8 && heap.ledger.live_bytes == 424);
    fuel = 138;
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0 && fuel == 0);
    const uint8_t zero[32] = {0}; assert(!memcmp(source, zero, 32));
    assert(value.length == 1 && oxid_bytebuf_validate(&heap, &child) == 0);
    int32_t count = -1; fuel = 19;
    assert(oxid_owned_vector_len(&heap, &value, 0, &fuel, &count) == 0 && count == 1 && fuel == 0);
    fuel = 19;
    assert(oxid_owned_vector_capacity(&heap, &value, 0, &fuel, &count) == 0 && count == 8 && fuel == 0);
    fuel = 106; memset(result, 0xff, 32);
    assert(oxid_owned_vector_pop_last(&heap, &value, 0, result, 32, &fuel) == 0 && fuel == 0);
    canonical(result, &child, 73);
    assert(value.length == 0 && heap.ledger.live_allocations == 2);
    fuel = 47;
    assert(oxid_owned_vector_take(&heap, &value, 0, &fuel, &moved) == 0 && fuel == 0);
    assert(value.identity == 0 && moved.capacity == 8);
    assert(oxid_owned_vector_release(&heap, &moved, 0) == 0);
    assert(oxid_bytebuf_validate(&heap, &child) == 0 && heap.ledger.releases == 1);
    assert(oxid_bytebuf_release(&heap, &child) == 0);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.allocations == heap.ledger.releases);
}
static void growth_failures_preserve_all_owners(void) {
    for (int existing = 0; existing <= 1; ++existing) {
        for (int boundary = 1; boundary <= 4; ++boundary) {
            oxid_byte_heap heap; oxid_heap_init(&heap);
            oxid_owned_vector value = {0, 0, 0};
            uint8_t source[32], saved[32];
            oxid_bytebuf child = incoming(&heap, source, 11);
            uint64_t fuel = enough();
            if (existing) {
                reserve(&heap, &value, 1);
                assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0);
            }
            memcpy(saved, source, 32);
            oxid_owned_vector before = value;
            uint64_t live = heap.ledger.live_bytes, nodes = heap.ledger.live_allocations;
            if (boundary == 4) force_null = 1;
            else heap.fault_boundary = boundary;
            fuel = enough();
            assert(oxid_owned_vector_reserve(&heap, &value, 0, 9, &fuel) == OXID_ALLOCATION_FAILED);
            force_null = 0;
            assert(!memcmp(&value, &before, sizeof(value)) && !memcmp(source, saved, 32));
            assert(heap.ledger.live_bytes == live && heap.ledger.live_allocations == nodes);
            assert(oxid_bytebuf_validate(&heap, &child) == 0);
            if (existing) {
                oxid_allocation *node; assert(locate_owned_vector(&heap, &value, owned_record_schema(0), &node) == 0);
                canonical(owned_vector_data(node), &child, 11);
            }
            reserve(&heap, &value, 9);
            assert(oxid_bytebuf_validate(&heap, &child) == 0);
            oxid_heap_cleanup(&heap);
            assert(heap.ledger.live_bytes == 0 && heap.ledger.allocations == heap.ledger.releases);
        }
    }
}
static void exact_and_one_less_work_is_atomic(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector value = {0, 0, 0}, result = {123, 4, 5}, saved_result = result;
    uint8_t source[32], saved_source[32], output[32], saved_output[32];
    oxid_bytebuf child = incoming(&heap, source, 9);
    memcpy(saved_source, source, 32);
    uint64_t fuel = 390;
    assert(oxid_owned_vector_reserve(&heap, &value, 0, 1, &fuel) == OXID_BUFFER_FUEL);
    assert(fuel == 390 && !value.identity && heap.ledger.live_allocations == 1);
    reserve(&heap, &value, 1);
    fuel = 137;
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == OXID_BUFFER_FUEL);
    assert(fuel == 137 && !value.length && !memcmp(source, saved_source, 32));
    fuel = enough(); assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0);
    memset(output, 0x6d, 32); memcpy(saved_output, output, 32);
    fuel = 105;
    assert(oxid_owned_vector_pop_last(&heap, &value, 0, output, 32, &fuel) == OXID_BUFFER_FUEL);
    assert(fuel == 105 && value.length == 1 && !memcmp(output, saved_output, 32));
    fuel = 72;
    assert(oxid_owned_vector_truncate(&heap, &value, 0, 0, &fuel) == OXID_BUFFER_FUEL);
    assert(fuel == 72 && value.length == 1 && oxid_bytebuf_validate(&heap, &child) == 0);
    fuel = 46;
    assert(oxid_owned_vector_take(&heap, &value, 0, &fuel, &result) == OXID_BUFFER_FUEL);
    assert(fuel == 46 && value.length == 1 && !memcmp(&result, &saved_result, sizeof(result)));
    int32_t count = -7; fuel = 18;
    assert(oxid_owned_vector_len(&heap, &value, 0, &fuel, &count) == OXID_BUFFER_FUEL);
    assert(fuel == 18 && count == -7);
    fuel = 73;
    assert(oxid_owned_vector_truncate(&heap, &value, 0, 0, &fuel) == 0 && fuel == 0);
    assert(value.length == 0 && oxid_bytebuf_validate(&heap, &child) == OXID_BUFFER_DESCRIPTOR);
    assert(heap.ledger.live_allocations == 1 && heap.ledger.releases == 1);
    assert(oxid_owned_vector_release(&heap, &value, 0) == 0 && heap.ledger.live_bytes == 0);
}
static void growth_drop_and_memory_limit(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector value = {0, 0, 0};
    oxid_bytebuf children[9]; uint8_t source[32]; uint64_t fuel;
    reserve(&heap, &value, 8);
    for (uint8_t i = 0; i < 8; ++i) {
        children[i] = incoming(&heap, source, i);
        fuel = enough(); assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0);
    }
    children[8] = incoming(&heap, source, 8);
    oxid_owned_vector before = value; uint8_t saved[32]; memcpy(saved, source, 32);
    fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == OXID_BUFFER_BOUNDS);
    assert(!memcmp(&before, &value, sizeof(value)) && !memcmp(source, saved, 32));
    heap.limit = heap.ledger.live_bytes; fuel = enough();
    assert(oxid_owned_vector_reserve(&heap, &value, 0, 1, &fuel) == OXID_MEMORY_LIMIT);
    assert(!memcmp(&before, &value, sizeof(value)) && !memcmp(source, saved, 32));
    heap.limit = OXID_BYTEBUF_LIMIT;
    reserve(&heap, &value, 1);
    assert(value.capacity == 16 && value.length == 8);
    fuel = enough(); assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0);
    fuel = enough(); assert(oxid_owned_vector_truncate(&heap, &value, 0, 3, &fuel) == 0);
    assert(value.length == 3 && heap.ledger.live_allocations == 4);
    for (int i = 0; i < 9; ++i)
        assert(oxid_bytebuf_validate(&heap, &children[i]) == (i < 3 ? 0 : OXID_BUFFER_DESCRIPTOR));
    assert(oxid_owned_vector_release(&heap, &value, 0) == 0);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.live_allocations == 0);
    assert(heap.ledger.allocations == heap.ledger.releases);
}
static void duplicate_and_bad_scalar_preserve_incoming(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector value = {0, 0, 0};
    uint8_t source[32], duplicate[32];
    oxid_bytebuf child = incoming(&heap, source, 31);
    memcpy(duplicate, source, 32); reserve(&heap, &value, 2);
    uint64_t fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0);
    uint8_t saved[32]; memcpy(saved, duplicate, 32); fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0, duplicate, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(value.length == 1 && !memcmp(saved, duplicate, 32));
    assert(oxid_bytebuf_validate(&heap, &child) == 0);
    oxid_bytebuf other = incoming(&heap, source, 32); source[0] = 2;
    memcpy(saved, source, 32); fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(saved, source, 32) && value.length == 1);
    assert(oxid_bytebuf_validate(&heap, &other) == 0);
    assert(oxid_owned_vector_release(&heap, &value, 0) == 0);
    assert(oxid_bytebuf_validate(&heap, &child) == OXID_BUFFER_DESCRIPTOR);
    assert(oxid_bytebuf_validate(&heap, &other) == 0);
    assert(oxid_bytebuf_release(&heap, &other) == 0 && heap.ledger.live_bytes == 0);
}
static void every_registered_kind_rejects_external_overlap(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    uint8_t source[32]; oxid_bytebuf child = incoming(&heap, source, 5);
    oxid_vector scalar = {0, 0, 0}, copy = {0, 0, 0};
    oxid_owned_vector value = {0, 0, 0}, other = {0, 0, 0};
    uint64_t fuel = enough(); const uint8_t word[4] = {0};
    assert(oxid_vector_push(&heap, &scalar, OXID_VECTOR_U32, word, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_push(&heap, &copy, COPY_RECORD_TAG, word, &fuel) == OXID_GROW_OK);
    reserve(&heap, &other, 1); reserve(&heap, &value, 1);
    int fd = open("/dev/null", O_RDONLY); assert(fd >= 0);
    oxid_allocation *file = malloc(32); assert(file);
    *file = (oxid_allocation){heap.head, NULL, heap.next_identity++, UINT32_MAX, (uint32_t)fd};
    heap.head->previous = file; heap.head = file;
    heap.ledger.live_bytes += 32; ++heap.ledger.live_allocations; ++heap.ledger.allocations;
    uint64_t live = heap.ledger.live_bytes, allocations = heap.ledger.allocations;
    size_t inspected = 0;
    for (oxid_allocation *node = heap.head; node; node = node->next) {
        ++inspected; fuel = enough();
        assert(oxid_owned_vector_push_reserved(&heap, &value, 0, (uint8_t *)node, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
        fuel = enough();
        assert(oxid_owned_vector_pop_last(&heap, &value, 0, (uint8_t *)node, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
        fuel = enough();
        assert(oxid_owned_vector_len(&heap, &value, 0, &fuel, (int32_t *)node) == OXID_BUFFER_DESCRIPTOR);
        fuel = enough();
        assert(oxid_owned_vector_take(&heap, &value, 0, &fuel, (oxid_owned_vector *)node) == OXID_BUFFER_DESCRIPTOR);
        fuel = enough();
        assert(oxid_owned_vector_reserve(&heap, (oxid_owned_vector *)node, 0, 1, &fuel) == OXID_BUFFER_DESCRIPTOR);
        assert(heap.ledger.live_bytes == live && heap.ledger.allocations == allocations);
    }
    assert(inspected == 6 && value.length == 0);
    fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &value, 0,
        (uint8_t *)(uintptr_t)(UINTPTR_MAX - 8u), 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
    fuel = enough();
    assert(oxid_owned_vector_take(&heap, &value, 0, &fuel, &value) == OXID_BUFFER_DESCRIPTOR);
    fuel = enough();
    assert(oxid_owned_vector_reserve(&heap, (oxid_owned_vector *)&heap, 0, 1, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(oxid_bytebuf_validate(&heap, &child) == 0);
    oxid_heap_cleanup(&heap);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.allocations == heap.ledger.releases);
    errno = 0; assert(fcntl(fd, F_GETFD) == -1 && errno == EBADF);
}
static void empty_nominal_and_capacity_denials(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector value = {0, 0, 0};
    uint8_t output[32], old[32]; memset(output, 0x27, 32); memcpy(old, output, 32);
    uint64_t fuel = enough();
    assert(oxid_owned_vector_pop_last(&heap, &value, 0, output, 32, &fuel) == OXID_BUFFER_BOUNDS);
    assert(!memcmp(output, old, 32));
    assert(oxid_owned_vector_reserve(&heap, &value, 0, -1, &fuel) == OXID_BUFFER_BOUNDS);
    assert(oxid_owned_vector_truncate(&heap, &value, 0, -1, &fuel) == OXID_BUFFER_BOUNDS);
    assert(heap.ledger.allocations == 0);
    reserve(&heap, &value, 1);
    int32_t count = -1; fuel = enough();
    assert(oxid_owned_vector_len(&heap, &value, 1, &fuel, &count) == OXID_BUFFER_DESCRIPTOR && count == -1);
    oxid_vector old_vector; oxid_bytebuf old_buffer;
    memcpy(&old_vector, &value, 16); memcpy(&old_buffer, &value, 16);
    assert(oxid_vector_validate(&heap, &old_vector, OXID_VECTOR_U32) == OXID_BUFFER_DESCRIPTOR);
    assert(oxid_bytebuf_validate(&heap, &old_buffer) == OXID_BUFFER_DESCRIPTOR);
    oxid_owned_vector before = value;
    value.length = 1; fuel = enough();
    assert(oxid_owned_vector_reserve(&heap, &value, 0, INT32_MAX, &fuel) == OXID_BUFFER_DESCRIPTOR);
    value = before;
    uint8_t source[32]; oxid_bytebuf child = incoming(&heap, source, 19);
    fuel = enough(); assert(oxid_owned_vector_push_reserved(&heap, &value, 0, source, 32, &fuel) == 0);
    before = value; fuel = enough();
    assert(oxid_owned_vector_reserve(&heap, &value, 0, INT32_MAX, &fuel) == OXID_CAPACITY_OVERFLOW);
    assert(!memcmp(&value, &before, sizeof(value)) && oxid_bytebuf_validate(&heap, &child) == 0);
    assert(oxid_owned_vector_release(&heap, &value, 0) == 0);
}
static void paid_descriptor_validation_is_atomic(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector value;
    assert(oxid_owned_vector_new(0, &value) == 0);
    uint64_t fuel = 3;
    assert(oxid_owned_vector_validate(&heap, &value, 0, &fuel) == OXID_BUFFER_FUEL && fuel == 3);
    fuel = 4;
    assert(oxid_owned_vector_validate(&heap, &value, 0, &fuel) == 0 && fuel == 0);
    reserve(&heap, &value, 1);
    const oxid_owned_vector before = value;
    assert(heap.ledger.live_allocations == 1);
    fuel = 6;
    assert(oxid_owned_vector_validate(&heap, &value, 0, &fuel) == OXID_BUFFER_FUEL && fuel == 6);
    fuel = 7;
    assert(oxid_owned_vector_validate(&heap, &value, 0, &fuel) == 0 && fuel == 0);
    fuel = 7;
    assert(oxid_owned_vector_validate(&heap, &value, 1, &fuel) == OXID_BUFFER_DESCRIPTOR && fuel == 7);
    fuel = 0;
    assert(oxid_owned_vector_validate_maintenance(&heap, &value, 0) == 0 && fuel == 0);
    assert(oxid_owned_vector_validate_maintenance(&heap, &value, 1) == OXID_BUFFER_DESCRIPTOR && fuel == 0);
    assert(oxid_owned_vector_validate(&heap, &value, 0, &heap.ledger.live_bytes) == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(&before, &value, sizeof(value)));
    assert(heap.ledger.live_bytes == 384 && heap.ledger.releases == 0);
    assert(oxid_owned_vector_release(&heap, &value, 0) == 0);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.live_allocations == 0);
}
int main(void) {
    move_round_trip_and_literal_work();
    growth_failures_preserve_all_owners();
    exact_and_one_less_work_is_atomic();
    growth_drop_and_memory_limit();
    duplicate_and_bad_scalar_preserve_incoming();
    every_registered_kind_rejects_external_overlap();
    empty_nominal_and_capacity_denials();
    paid_descriptor_validation_is_atomic();
    puts("owned vector operations: 8 component groups passed");
    return 0;
}
