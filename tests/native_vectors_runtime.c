/* Standalone acceptance for the typed vector storage ABI.
 * No generated Oxid emitter or implementation-derived value oracle is used.
 */
#define _POSIX_C_SOURCE 200809L
#define OXID_BYTEBUF_TESTING
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int force_null;
static void *vector_test_allocate(size_t alignment, size_t length) {
    return force_null ? NULL : aligned_alloc(alignment, length);
}
#define OXID_VECTOR_ALIGNED_ALLOC vector_test_allocate
/* The integrated core includes experimental_vector.c once at its end. */
#include "../native/experimental_bytebuf.c"

static uint64_t enough(void) { return UINT64_C(1000000); }
static void initialized_bytes(oxid_byte_heap *heap, const oxid_vector *value,
    uint32_t element, const uint8_t *expected, size_t bytes) {
    uint8_t *actual = NULL;
    assert(oxid_vector_range(heap, value, element, 0, value->length, &actual) == 0);
    assert(!bytes || !memcmp(actual, expected, bytes));
}
static void scalar_oracles(void) {
    const struct { uint32_t tag; size_t width; uint64_t charge; uint8_t bytes[16]; } cases[] = {
        {OXID_VECTOR_BOOL, 1, 64, {1}}, {OXID_VECTOR_UNIT, 1, 64, {0}},
        {OXID_VECTOR_I8, 1, 64, {0x80}}, {OXID_VECTOR_U8, 1, 64, {0xff}},
        {OXID_VECTOR_I16, 2, 64, {0,0x80}}, {OXID_VECTOR_U16, 2, 64, {0xab,0xfe}},
        {OXID_VECTOR_I32, 4, 80, {0,0,0,0x80}}, {OXID_VECTOR_U32, 4, 80, {255,255,255,255}},
        {OXID_VECTOR_I64, 8, 112, {0,0,0,0,0,0,0,0x80}},
        {OXID_VECTOR_U64, 8, 112, {255,255,255,255,255,255,255,255}},
        {OXID_VECTOR_ISIZE, 8, 112, {0,0,0,0,0,0,0,0x80}},
        {OXID_VECTOR_USIZE, 8, 112, {255,255,255,255,255,255,255,255}},
        {OXID_VECTOR_F32, 4, 80, {0,0,0,0x80}},
        {OXID_VECTOR_F64, 8, 112, {0x42,0,0,0,0,0,0xf8,0x7f}},
        {OXID_VECTOR_CHAR, 4, 80, {255,255,0x10,0}},
        {OXID_VECTOR_I128, 16, 176, {0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0x80}},
        {OXID_VECTOR_U128, 16, 176, {255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255}}
    };
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        oxid_byte_heap heap;
        oxid_heap_init(&heap);
        oxid_vector value, taken, moved = {0,0,0};
        assert(oxid_vector_new(cases[i].tag, &value) == 0);
        uint64_t fuel = enough();
        assert(oxid_vector_push(&heap, &value, cases[i].tag, cases[i].bytes, &fuel) == OXID_GROW_OK);
        assert(value.length == 1 && value.capacity == 8);
        assert(heap.ledger.live_bytes == cases[i].charge);
        int32_t count = -1;
        assert(oxid_vector_len(&heap, &value, cases[i].tag, &count) == 0 && count == 1);
        assert(oxid_vector_capacity(&heap, &value, cases[i].tag, &count) == 0 && count == 8);
        uint8_t *bytes = NULL;
        assert(oxid_vector_range(&heap, &value, cases[i].tag, 0, 1, &bytes) == 0);
        assert((uintptr_t)bytes % 16u == 0);
        assert(!memcmp(bytes, cases[i].bytes, cases[i].width));
        heap.limit = heap.ledger.live_bytes;
        assert(oxid_vector_take(&heap, &value, cases[i].tag, &taken) == 0 && !value.identity);
        assert(oxid_vector_move(&heap, &taken, cases[i].tag, &moved) == 0 && !taken.identity);
        initialized_bytes(&heap, &moved, cases[i].tag, cases[i].bytes, cases[i].width);
        assert(oxid_vector_release(&heap, &moved, cases[i].tag) == 0);
        assert(!heap.ledger.live_bytes && heap.ledger.allocations == heap.ledger.releases);
    }
}
static void float_patterns(void) {
    const uint64_t words[] = {UINT64_C(0),UINT64_C(0x8000000000000000),
        UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),
        UINT64_C(0x7ff8000000000042),UINT64_C(0x7ff0000000000001)};
    for (size_t i = 0; i < sizeof(words) / sizeof(words[0]); ++i) {
        uint8_t scalar[8];
        for (size_t b = 0; b < 8; ++b) scalar[b] = (uint8_t)(words[i] >> (8*b));
        oxid_byte_heap heap;
        oxid_heap_init(&heap);
        oxid_vector value = {0,0,0};
        uint64_t fuel = enough();
        assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_F64, 9, scalar, &fuel) == OXID_GROW_OK);
        assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_F64, 20, &fuel) == OXID_GROW_OK);
        uint8_t *actual;
        assert(oxid_vector_range(&heap, &value, OXID_VECTOR_F64, 0, 9, &actual) == 0);
        for (size_t cell = 0; cell < 9; ++cell) assert(!memcmp(actual + cell*8, scalar, 8));
        oxid_heap_cleanup(&heap);
        assert(!heap.ledger.live_bytes && heap.ledger.allocations == heap.ledger.releases);
    }
    const uint32_t narrow[] = {UINT32_C(0),UINT32_C(0x80000000),
        UINT32_C(0x7f800000),UINT32_C(0xff800000),UINT32_C(0x7fc00042),UINT32_C(0x7f800001)};
    for (size_t i = 0; i < sizeof(narrow) / sizeof(narrow[0]); ++i) {
        uint8_t scalar[4];
        for (size_t b = 0; b < 4; ++b) scalar[b] = (uint8_t)(narrow[i] >> (8*b));
        oxid_byte_heap heap;
        oxid_heap_init(&heap);
        oxid_vector value = {0,0,0};
        uint64_t fuel = enough();
        assert(oxid_vector_push(&heap, &value, OXID_VECTOR_F32, scalar, &fuel) == OXID_GROW_OK);
        initialized_bytes(&heap, &value, OXID_VECTOR_F32, scalar, 4);
        oxid_heap_cleanup(&heap);
    }
}
static void failure_preservation(void) {
    uint8_t scalar[16] = {0};
    scalar[15] = 0x80;
    for (int boundary = 1; boundary <= 3; ++boundary) {
        for (int existing = 0; existing <= 1; ++existing) {
            oxid_byte_heap heap;
            oxid_heap_init(&heap);
            oxid_vector value = {0,0,0};
            uint64_t fuel = enough();
            if (existing) assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_I128, 8, scalar, &fuel) == OXID_GROW_OK);
            oxid_vector before = value;
            uint64_t live = heap.ledger.live_bytes, allocations = heap.ledger.live_allocations;
            heap.fault_boundary = boundary;
            assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_I128, 9, scalar, &fuel) == OXID_ALLOCATION_FAILED);
            assert(!memcmp(&value, &before, sizeof(value)));
            assert(heap.ledger.live_bytes == live && heap.ledger.live_allocations == allocations);
            if (existing) {
                uint8_t *bytes;
                assert(oxid_vector_range(&heap, &value, OXID_VECTOR_I128, 0, 8, &bytes) == 0);
                for (size_t cell = 0; cell < 8; ++cell) assert(!memcmp(bytes + cell*16, scalar, 16));
            }
            assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_I128, 9, scalar, &fuel) == OXID_GROW_OK);
            uint8_t *bytes;
            assert(oxid_vector_range(&heap, &value, OXID_VECTOR_I128, 0, 9, &bytes) == 0);
            assert((uintptr_t)bytes % 16u == 0);
            for (size_t cell = 0; cell < 9; ++cell) assert(!memcmp(bytes + cell*16, scalar, 16));
            oxid_heap_cleanup(&heap);
            assert(!heap.ledger.live_bytes && heap.ledger.allocations == heap.ledger.releases);
        }
    }
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_vector value = {0,0,0};
    uint64_t fuel = enough();
    force_null = 1;
    assert(oxid_vector_push(&heap, &value, OXID_VECTOR_I128, scalar, &fuel) == OXID_ALLOCATION_FAILED);
    force_null = 0;
    assert(!value.identity && !heap.head && !heap.ledger.allocations && heap.next_identity == 1);
    assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_I128, 3, scalar, &fuel) == OXID_GROW_OK);
    oxid_vector before = value;
    heap.fault_boundary = COMMIT;
    assert(oxid_vector_push(&heap, &value, OXID_VECTOR_I128, scalar, &fuel) == OXID_ALLOCATION_FAILED);
    assert(!memcmp(&value, &before, sizeof(value)));
    heap.fault_boundary = COMMIT;
    assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_I128, 1, scalar, &fuel) == OXID_ALLOCATION_FAILED);
    assert(!memcmp(&value, &before, sizeof(value)));
    oxid_heap_cleanup(&heap);
}
static void peak_and_validation(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    heap.limit = 479;
    oxid_vector value = {0,0,0};
    uint8_t zero[16] = {0};
    uint64_t fuel = 144;
    assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_I128, 8, zero, &fuel) == OXID_GROW_OK && fuel == 0);
    assert(heap.ledger.live_bytes == 176);
    oxid_vector before = value;
    fuel = 128;
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_I128, 1, &fuel) == OXID_MEMORY_LIMIT && fuel == 128);
    assert(!memcmp(&value, &before, sizeof(value)));
    heap.limit = 480;
    fuel = 127;
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_I128, 1, &fuel) == OXID_BUFFER_FUEL && fuel == 127);
    assert(!memcmp(&value, &before, sizeof(value)));
    fuel = 128;
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_I128, 1, &fuel) == OXID_GROW_OK && fuel == 0);
    assert(heap.ledger.live_bytes == 304 && heap.ledger.peak_bytes == 480);
    before = value;
    assert(oxid_vector_validate(&heap, &value, OXID_VECTOR_U128) == OXID_BUFFER_DESCRIPTOR);
    assert(oxid_vector_validate(&heap, &value, 0) == OXID_BUFFER_DESCRIPTOR);
    fuel = enough();
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_I128, INT32_MAX, &fuel) == OXID_CAPACITY_OVERFLOW);
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_I128, -1, &fuel) == OXID_BUFFER_BOUNDS);
    assert(oxid_vector_extend(&heap, &value, OXID_VECTOR_I128, NULL, -1, &fuel) == OXID_BUFFER_BOUNDS);
    assert(!memcmp(&value, &before, sizeof(value)) && fuel == enough());
    uint8_t *sentinel = zero;
    assert(oxid_vector_range(&heap, &value, OXID_VECTOR_I128, 0, 9, &sentinel) == OXID_BUFFER_BOUNDS && sentinel == zero);
    uint8_t *bytes;
    assert(oxid_vector_range(&heap, &value, OXID_VECTOR_I128, 0, 1, &bytes) == 0);
    assert(oxid_vector_extend(&heap, &value, OXID_VECTOR_I128, bytes, 1, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(oxid_vector_take(&heap, &value, OXID_VECTOR_I128, &value) == OXID_BUFFER_DESCRIPTOR);
    oxid_vector wrong = value;
    wrong.length = -1;
    assert(oxid_vector_validate(&heap, &wrong, OXID_VECTOR_I128) == OXID_BUFFER_DESCRIPTOR);
    oxid_bytebuf disguised = {value.identity, value.length, value.capacity};
    assert(oxid_bytebuf_validate(&heap, &disguised) == OXID_BUFFER_DESCRIPTOR);
    oxid_heap_cleanup(&heap);
    assert(oxid_vector_validate(&heap, &value, OXID_VECTOR_I128) == OXID_BUFFER_DESCRIPTOR);
    assert(!heap.ledger.live_bytes && heap.ledger.allocations == heap.ledger.releases);

    const struct { uint32_t tag; uint8_t bytes[4]; } invalid[] = {
        {OXID_VECTOR_BOOL,{2}}, {OXID_VECTOR_UNIT,{1}},
        {OXID_VECTOR_CHAR,{0,0xd8,0,0}}, {OXID_VECTOR_CHAR,{0,0,0x11,0}}
    };
    for (size_t i = 0; i < sizeof(invalid)/sizeof(invalid[0]); ++i) {
        oxid_heap_init(&heap);
        value = (oxid_vector){0,0,0};
        fuel = 100;
        assert(oxid_vector_push(&heap, &value, invalid[i].tag, invalid[i].bytes, &fuel) == OXID_BUFFER_DESCRIPTOR);
        assert(!value.identity && !heap.ledger.live_bytes && fuel == 100);
    }
    oxid_heap_init(&heap);
    oxid_bytebuf old = {0,0,0};
    fuel = enough();
    assert(oxid_bytebuf_push(&heap, &old, 7, &fuel) == OXID_GROW_OK);
    value = (oxid_vector){old.identity,old.length,old.capacity};
    assert(oxid_vector_validate(&heap, &value, OXID_VECTOR_U8) == OXID_BUFFER_DESCRIPTOR);
    oxid_heap_cleanup(&heap);
    assert(!heap.ledger.live_bytes && heap.ledger.allocations == heap.ledger.releases);
}
static void distinct_sources_and_truncation(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_vector source = {0,0,0}, target = {0,0,0};
    const uint8_t input[] = {1,0,2,0,3,0};
    const uint8_t wanted[] = {2,0,3,0};
    uint64_t fuel = enough();
    assert(oxid_vector_extend(&heap, &source, OXID_VECTOR_U16, input, 3, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_extend_vector(&heap, &target, OXID_VECTOR_U16, &source, 1, 3, &fuel) == OXID_GROW_OK);
    initialized_bytes(&heap, &target, OXID_VECTOR_U16, wanted, 4);
    assert(oxid_vector_extend_vector(&heap, &target, OXID_VECTOR_U16, &target, 0, 1, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(oxid_vector_move(&heap, &source, OXID_VECTOR_U16, &target) == OXID_BUFFER_DESCRIPTOR);
    assert(source.length == 3 && target.length == 2);
    assert(oxid_vector_truncate(&heap, &target, OXID_VECTOR_U16, 1) == 0);
    const uint8_t fill[] = {0xef,0xbe};
    const uint8_t resized[] = {2,0,0xef,0xbe,0xef,0xbe};
    assert(oxid_vector_resize(&heap, &target, OXID_VECTOR_U16, 3, fill, &fuel) == OXID_GROW_OK);
    initialized_bytes(&heap, &target, OXID_VECTOR_U16, resized, sizeof(resized));
    oxid_bytebuf bytes = {0,0,0};
    assert(oxid_bytebuf_push(&heap, &bytes, 99, &fuel) == OXID_GROW_OK);
    /* Mixed registry cleanup must derive the class from each live node. */
    oxid_heap_cleanup(&heap);
    assert(!heap.ledger.live_bytes && !heap.ledger.live_allocations);
    assert(heap.ledger.allocations == heap.ledger.releases);
}
/* Private-parameter regression: no source-language vector call exists yet.
 * Run this case alone by passing "fuel-alias" to the component executable. */
static void vector_fuel_alias_refuses_before_mutation(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_vector value = {0,0,0};
    const uint8_t thousand[] = {0xe8,0x03,0,0,0,0,0,0};
    uint64_t fuel = enough();
    assert(oxid_vector_resize(&heap, &value, OXID_VECTOR_U64, 8, thousand, &fuel) == OXID_GROW_OK);
    uint8_t *bytes;
    assert(oxid_vector_range(&heap, &value, OXID_VECTOR_U64, 0, 8, &bytes) == 0);
    uint8_t before[64];
    memcpy(before, bytes, sizeof(before));
    oxid_vector descriptor = value;
    heap.fault_boundary = ALLOCATE;
    int32_t status = oxid_vector_reserve(&heap, &value, OXID_VECTOR_U64, 1, (uint64_t *)bytes);
    if (status != OXID_BUFFER_DESCRIPTOR) {
        uint64_t observed = 0;
        for (size_t i = 0; i < 8; ++i) observed |= (uint64_t)bytes[i] << (i*8);
        fprintf(stderr, "private vector fuel alias: status=%" PRId32 ", first=%" PRIu64 " (expected -3, 1000)\n", status, observed);
    }
    assert(status == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(&value, &descriptor, sizeof(value)) && !memcmp(bytes, before, sizeof(before)));
    assert(heap.fault_boundary == ALLOCATE);
    assert(heap.ledger.live_bytes == 112 && heap.ledger.live_allocations == 1);
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_U64, 1, &heap.ledger.live_bytes) == OXID_BUFFER_DESCRIPTOR);
    assert(heap.ledger.live_bytes == 112);
    oxid_heap_cleanup(&heap);
    assert(!heap.ledger.live_bytes && heap.ledger.allocations == heap.ledger.releases);
}
static void identity_exhaustion(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    heap.next_identity = UINT64_MAX;
    heap.fault_boundary = ALLOCATE;
    oxid_vector value = {0,0,0};
    uint64_t fuel = 100;
    assert(oxid_vector_reserve(&heap, &value, OXID_VECTOR_U8, 1, &fuel) == OXID_BUFFER_IDENTITY);
    assert(!value.identity && !heap.head && !heap.ledger.live_bytes && fuel == 100);
    assert(heap.next_identity == UINT64_MAX && heap.fault_boundary == ALLOCATE);
}
int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "fuel-alias")) {
        vector_fuel_alias_refuses_before_mutation();
        return 0;
    }
    scalar_oracles();
    float_patterns();
    failure_preservation();
    peak_and_validation();
    distinct_sources_and_truncation();
    vector_fuel_alias_refuses_before_mutation();
    identity_exhaustion();
    return 0;
}
