/* Independent component controls for active owned-record cell encoding. */
#define _POSIX_C_SOURCE 200809L
#define OXID_RECORD_VECTOR_SCHEMAS
#define OXID_OWNED_VECTOR_SCHEMAS
#ifdef OXID_TEST_OWNED_VECTOR_COMPOSED_OPERATIONS
#define OXID_OWNED_VECTOR_RUNTIME
#define OXID_BYTEBUF_TESTING
#endif
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "../native/experimental_bytebuf.c"
#ifdef OXID_TEST_OWNED_VECTOR_COMPOSED_OPERATIONS
#include "../native/experimental_owned_vector.c"
#else
#include "../native/experimental_owned_record.c"
#endif

static const oxid_vector_record_leaf copy_leaves[] = {{0, 0, OXID_VECTOR_U32}};
#define COPY_RECORD_TAG (OXID_VECTOR_RECORD_BIT | 2u)
const uint32_t __oxid_record_vector_schema_count = 3;
const oxid_vector_record_schema __oxid_record_vector_schemas[] = {
    {0, 0, 0, 0, NULL}, {0, 0, 0, 0, NULL},
    {COPY_RECORD_TAG, 4, 4, 1, copy_leaves}
};
#ifndef OXID_TEST_GENERATED_OWNED_SCHEMAS
const oxid_owned_type __oxid_owned_types[] = {
    {OXID_OWNED_SCALAR, OXID_VECTOR_U32, 0, 0, 4},
    {OXID_OWNED_SCALAR, OXID_VECTOR_BOOL, 0, 0, 1},
    {OXID_OWNED_SCALAR, OXID_VECTOR_CHAR, 0, 0, 4},
    {OXID_OWNED_BYTEBUF, 0, 0, 0, 16},
    {OXID_OWNED_FILE, 0, 0, 0, 16},
    {OXID_OWNED_VECTOR, OXID_VECTOR_U32, 0, 0, 16},
    {OXID_OWNED_VECTOR, COPY_RECORD_TAG, 0, 0, 16},
    {OXID_OWNED_ENUM, 0, 0, 3, 32},
    {OXID_OWNED_SCALAR, OXID_VECTOR_I128, 0, 0, 16},
    {OXID_OWNED_RECORD, 0, 0, 0, 1},
    {OXID_OWNED_ARRAY, OXID_VECTOR_U16, 0, 0, 2},
    {OXID_OWNED_RECORD, 0, 0, 8, 112},
    {OXID_OWNED_RECORD, 0, 8, 2, 32},
    {OXID_OWNED_RECORD, 0, 10, 1, 16},
    {OXID_OWNED_RECORD, 0, 11, 1, 16},
    {OXID_OWNED_RECORD, 0, UINT32_MAX, 1, 16},
    {OXID_OWNED_ENUM, 0, UINT32_MAX, 2, 16},
    {OXID_OWNED_RECORD, 0, 12, 1, 16}
};
const oxid_owned_field __oxid_owned_fields[] = {
    {1, 0}, {2, 4}, {7, 16}, {4, 48}, {5, 64}, {6, 80}, {9, 96}, {10, 98},
    {3, 0}, {3, 16}, {3, 1}, {14, 0}, {16, 0}
};
const oxid_owned_field __oxid_owned_variants[] = {
    {UINT32_MAX, 0}, {3, 16}, {8, 16}
};
const oxid_owned_record_schema __oxid_owned_records[] = {
    {0, 11, 112, 20, 4}, {1, 12, 32, 5, 2},
    {2, UINT32_MAX, 32, 1, 0}, {3, 13, 16, 3, 1},
    {4, 14, 16, 100, 0}, {5, 15, 16, 3, 0}, {6, 17, 16, 4, 0}
};
const uint32_t __oxid_owned_type_count = sizeof(__oxid_owned_types) / sizeof(__oxid_owned_types[0]);
const uint32_t __oxid_owned_field_count = sizeof(__oxid_owned_fields) / sizeof(__oxid_owned_fields[0]);
const uint32_t __oxid_owned_variant_count = 3;
const uint32_t __oxid_owned_record_count = sizeof(__oxid_owned_records) / sizeof(__oxid_owned_records[0]);
#endif

static void source_record(uint8_t bytes[112], uint32_t tag) {
    memset(bytes, 0xa5, 112);
    bytes[0] = 1;
    const uint8_t character[] = {0, 0xf6, 1, 0}; /* U+1F600 */
    memcpy(bytes + 4, character, 4);
    memset(bytes + 16, 0, 4);
    bytes[16] = (uint8_t)tag;
    if (tag == 1) memset(bytes + 32, 0, 16);
    if (tag == 2) for (size_t i = 0; i < 16; ++i) bytes[32 + i] = (uint8_t)(0xf0u + i);
    memset(bytes + 48, 0, 48);
    bytes[96] = 0;
    bytes[98] = 0x34; bytes[99] = 0x12;
}
static void active_cells_and_padding(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    const oxid_owned_record_schema *schema = owned_record_schema(0);
    assert(schema && !owned_record_schema(2) && !owned_record_schema(99));
    for (uint32_t tag = 0; tag < 3; ++tag) {
        uint8_t source[112], actual[112], expected[112] = {0};
        source_record(source, tag);
        expected[0] = 1;
        expected[5] = 0xf6; expected[6] = 1;
        expected[16] = (uint8_t)tag;
        expected[98] = 0x34; expected[99] = 0x12;
        if (tag == 2) for (size_t i = 0; i < 16; ++i) expected[32 + i] = (uint8_t)(0xf0u + i);
        uint64_t identities[4]; size_t used = 0;
        assert(owned_record_check(&heap, schema, source, 112, identities, 4, &used));
        assert(used == 0);
        memset(actual, 0xff, 112);
        assert(owned_record_copy(schema, source, actual, 112));
        assert(!memcmp(actual, expected, 112));
        assert(source[1] == 0xa5 && source[97] == 0xa5 && source[111] == 0xa5);
    }
}
static void invalid_values_and_tables_are_refused(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    uint8_t source[112], destination[112], old[112];
    uint64_t identities[4]; size_t used;
    const oxid_owned_record_schema *schema = owned_record_schema(0);
    for (int variant = 0; variant < 7; ++variant) {
        source_record(source, 0);
        switch (variant) {
        case 0: source[0] = 2; break;
        case 1: source[4] = 0; source[5] = 0xd8; source[6] = 0; break;
        case 2: source[4] = 0; source[5] = 0; source[6] = 0x11; break;
        case 3: source[16] = 3; break;
        case 4: source[19] = 0xff; break;
        case 5: source[96] = 1; break;
        case 6: source[56] = 1; break; /* File reserved word. */
        }
        used = 0;
        assert(!owned_record_check(&heap, schema, source, 112, identities, 4, &used));
        assert(heap.ledger.live_bytes == 0);
    }
    source_record(source, 0);
    for (uint32_t record = 3; record <= 6; ++record) {
        used = 0;
        assert(!owned_record_check(&heap, owned_record_schema(record), source, 16, identities, 4, &used));
    }
    source_record(source, 3);
    memset(destination, 0x71, 112); memcpy(old, destination, 112);
    assert(!owned_record_copy(schema, source, destination, 112));
    assert(!memcmp(destination, old, 112));
    source_record(source, 0); used = 0;
    assert(!owned_record_check(&heap, schema, source, 111, identities, 4, &used));
    assert(!owned_record_check(&heap, schema, NULL, 112, identities, 4, &used));
}
static int file_fixture(oxid_byte_heap *heap, uint8_t bytes[16]) {
    int fd = open("/dev/null", O_RDONLY);
    assert(fd >= 0);
    oxid_allocation *node = malloc(32);
    assert(node);
    *node = (oxid_allocation){heap->head, NULL, heap->next_identity++, UINT32_MAX, (uint32_t)fd};
    if (heap->head) heap->head->previous = node;
    heap->head = node;
    heap->ledger.live_bytes += 32;
    ++heap->ledger.live_allocations; ++heap->ledger.allocations;
    memset(bytes, 0, 16); memcpy(bytes, &node->identity, 8);
    return fd;
}
static void all_resource_kinds_release_once(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_bytebuf bytes = {0, 0, 0};
    oxid_vector scalar = {0, 0, 0}, copy = {0, 0, 0};
    uint64_t fuel = 1000000;
    const uint8_t value[] = {4, 3, 2, 1};
    assert(oxid_bytebuf_extend(&heap, &bytes, value, 4, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_push(&heap, &scalar, OXID_VECTOR_U32, value, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_push(&heap, &copy, COPY_RECORD_TAG, value, &fuel) == OXID_GROW_OK);
    uint8_t source[112], actual[112]; source_record(source, 1);
    memcpy(source + 32, &bytes, 16);
    int fd = file_fixture(&heap, source + 48);
    memcpy(source + 64, &scalar, 16); memcpy(source + 80, &copy, 16);
    assert(heap.ledger.live_bytes == 232 && heap.ledger.live_allocations == 4);
    uint64_t identities[4]; size_t used = 0;
    const oxid_owned_record_schema *schema = owned_record_schema(0);
    assert(owned_record_check(&heap, schema, source, 112, identities, 4, &used));
    assert(used == 4 && owned_record_unique(identities, used));
    assert(owned_record_copy(schema, source, actual, 112));
    used = 0;
    assert(!owned_record_check(&heap, schema, source, 112, identities, 3, &used));
    assert(heap.ledger.live_allocations == 4 && heap.ledger.releases == 0);
    uint8_t saved[16]; memcpy(saved, source + 48, 16);
    memcpy(source + 48, &bytes, 16); used = 0;
    assert(!owned_record_check(&heap, schema, source, 112, identities, 4, &used));
    memcpy(source + 48, saved, 16);
    used = 0;
    assert(owned_record_check(&heap, schema, source, 112, identities, 4, &used));
    owned_record_release(&heap, schema, actual, 112);
    assert(heap.head == NULL && heap.ledger.live_bytes == 0 && heap.ledger.live_allocations == 0);
    assert(heap.ledger.allocations == 4 && heap.ledger.releases == 4);
    errno = 0; assert(fcntl(fd, F_GETFD) == -1 && errno == EBADF);
    used = 0;
    assert(!owned_record_check(&heap, schema, source, 112, identities, 4, &used));
}
static void inactive_payloads_do_not_own_resources(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    uint8_t source[112], canonical[112]; source_record(source, 0);
    oxid_bytebuf bytes = {0, 0, 0}; uint64_t fuel = 1000000;
    assert(oxid_bytebuf_push(&heap, &bytes, 13, &fuel) == OXID_GROW_OK);
    memcpy(source + 32, &bytes, 16);
    uint64_t identities[4]; size_t used = 0;
    const oxid_owned_record_schema *schema = owned_record_schema(0);
    assert(owned_record_check(&heap, schema, source, 112, identities, 4, &used) && used == 0);
    assert(owned_record_copy(schema, source, canonical, 112));
    const uint8_t zero[16] = {0}; assert(!memcmp(canonical + 32, zero, 16));
    owned_record_release(&heap, schema, source, 112);
    assert(heap.ledger.live_allocations == 1 && heap.ledger.releases == 0);
    assert(oxid_bytebuf_validate(&heap, &bytes) == 0);
    assert(oxid_bytebuf_release(&heap, &bytes) == 0);
}
static void duplicate_owners_and_sort_bounds(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_bytebuf bytes = {0, 0, 0}; uint64_t fuel = 1000000;
    assert(oxid_bytebuf_push(&heap, &bytes, 42, &fuel) == OXID_GROW_OK);
    uint8_t source[32]; memcpy(source, &bytes, 16); memcpy(source + 16, &bytes, 16);
    uint64_t identities[2]; size_t used = 0;
    assert(owned_record_check(&heap, owned_record_schema(1), source, 32, identities, 2, &used));
    assert(used == 2 && !owned_record_unique(identities, used));
    assert(heap.ledger.releases == 0 && oxid_bytebuf_validate(&heap, &bytes) == 0);
    uint64_t shuffled[257];
    for (size_t count = 0; count <= 257; ++count) {
        for (size_t i = 0; i < count; ++i) shuffled[i] = (uint64_t)((i * 193u) % 257u) + 1u;
        assert(owned_record_unique(shuffled, count));
        for (size_t i = 1; i < count; ++i) assert(shuffled[i - 1] < shuffled[i]);
    }
    shuffled[0] = UINT64_MAX; shuffled[1] = UINT64_MAX;
    assert(!owned_record_unique(shuffled, 2));
    assert(owned_record_unique(NULL, 0) && !owned_record_unique(NULL, 1));
    oxid_heap_cleanup(&heap);
    assert(heap.ledger.releases == 1 && heap.ledger.live_bytes == 0);
}
#ifdef OXID_TEST_OWNED_VECTOR_COMPOSED_OPERATIONS
static int complete_record(oxid_byte_heap *heap, uint8_t source[112], uint8_t value) {
    source_record(source, 1);
    uint64_t fuel = 1000000;
    oxid_bytebuf bytes = {0, 0, 0};
    oxid_vector scalar = {0, 0, 0}, copy = {0, 0, 0};
    uint8_t word[] = {value, 0, 0, 0};
    assert(oxid_bytebuf_extend(heap, &bytes, word, 4, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_push(heap, &scalar, OXID_VECTOR_U32, word, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_push(heap, &copy, COPY_RECORD_TAG, word, &fuel) == OXID_GROW_OK);
    memcpy(source + 32, &bytes, 16);
    memcpy(source + 64, &scalar, 16); memcpy(source + 80, &copy, 16);
    return file_fixture(heap, source + 48);
}
static void composed_owned_vectors_transfer_and_drop_only_active_resources(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector vector = {0, 0, 0};
    uint8_t first[112], second[112], result[112];
    int first_fd = complete_record(&heap, first, 17);
    int second_fd = complete_record(&heap, second, 29);
    uint64_t fuel = 1000000;
    assert(oxid_owned_vector_reserve(&heap, &vector, 0, 2, &fuel) == OXID_GROW_OK);
    assert(heap.ledger.live_bytes == 1680 && vector.capacity == 8);
    assert(oxid_owned_vector_push_reserved(&heap, &vector, 0, first, 112, &fuel) == 0);
    assert(oxid_owned_vector_push_reserved(&heap, &vector, 0, second, 112, &fuel) == 0);
    assert(vector.length == 2 && heap.ledger.live_allocations == 9);
    const uint8_t zero[112] = {0};
    assert(!memcmp(first, zero, 112) && !memcmp(second, zero, 112));
    oxid_owned_vector before = vector;
    heap.fault_boundary = COMMIT;
    assert(oxid_owned_vector_reserve(&heap, &vector, 0, 7, &fuel) == OXID_ALLOCATION_FAILED);
    assert(!memcmp(&before, &vector, sizeof(vector)));
    assert(heap.ledger.live_bytes == 1680 && heap.ledger.live_allocations == 9);
    assert(fcntl(first_fd, F_GETFD) >= 0 && fcntl(second_fd, F_GETFD) >= 0);
    assert(oxid_owned_vector_reserve(&heap, &vector, 0, 7, &fuel) == OXID_GROW_OK);
    assert(vector.capacity == 16 && heap.ledger.live_bytes == 2832);
    memset(result, 0xff, 112);
    assert(oxid_owned_vector_pop_last(&heap, &vector, 0, result, 112, &fuel) == 0);
    assert(vector.length == 1 && heap.ledger.live_allocations == 9);
    assert(oxid_owned_vector_truncate(&heap, &vector, 0, 0, &fuel) == 0);
    errno = 0; assert(fcntl(first_fd, F_GETFD) == -1 && errno == EBADF);
    assert(fcntl(second_fd, F_GETFD) >= 0 && heap.ledger.live_allocations == 5);
    assert(oxid_owned_vector_release(&heap, &vector, 0) == 0);
    assert(heap.ledger.live_allocations == 4 && fcntl(second_fd, F_GETFD) >= 0);
    owned_record_release(&heap, owned_record_schema(0), result, 112);
    errno = 0; assert(fcntl(second_fd, F_GETFD) == -1 && errno == EBADF);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.allocations == heap.ledger.releases);
    source_record(first, 0);
    oxid_bytebuf inactive = {0, 0, 0}; fuel = 1000000;
    assert(oxid_bytebuf_push(&heap, &inactive, 51, &fuel) == OXID_GROW_OK);
    memcpy(first + 32, &inactive, 16);
    assert(oxid_owned_vector_reserve(&heap, &vector, 0, 1, &fuel) == OXID_GROW_OK);
    assert(oxid_owned_vector_push_reserved(&heap, &vector, 0, first, 112, &fuel) == 0);
    assert(oxid_owned_vector_truncate(&heap, &vector, 0, 0, &fuel) == 0);
    assert(oxid_owned_vector_release(&heap, &vector, 0) == 0);
    assert(oxid_bytebuf_validate(&heap, &inactive) == 0 && heap.ledger.live_allocations == 1);
    assert(oxid_bytebuf_release(&heap, &inactive) == 0 && heap.ledger.live_bytes == 0);
}
#endif
int main(void) {
    active_cells_and_padding();
    invalid_values_and_tables_are_refused();
    all_resource_kinds_release_once();
    inactive_payloads_do_not_own_resources();
    duplicate_owners_and_sort_bounds();
#ifdef OXID_TEST_OWNED_VECTOR_COMPOSED_OPERATIONS
    composed_owned_vectors_transfer_and_drop_only_active_resources();
    puts("owned record codec and vector components: 6 passed");
#else
    puts("owned record codec components: 5 passed");
#endif
    return 0;
}
