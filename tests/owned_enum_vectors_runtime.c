/* Distinct enum-root authentication and active-cell ownership controls. */
#define _POSIX_C_SOURCE 200809L
#define OXID_BYTEBUF_TESTING
#define OXID_OWNED_VECTOR_RUNTIME
#define OXID_OWNED_VECTOR_SCHEMAS
#define OXID_OWNED_ENUM_VECTOR_SCHEMAS
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "../native/experimental_bytebuf.c"
#include "../native/experimental_owned_vector.c"

#define ENUM_ROOT UINT32_C(0x80000000)
#ifndef OXID_TEST_GENERATED_OWNED_ENUM_SCHEMAS
/* Deliberately malformed nominal rows exercise failures that a checked
 * compiler never emits. The good record and enum have identical byte size. */
const oxid_owned_type __oxid_owned_types[] = {
    {OXID_OWNED_RECORD, 0, 0, 2, 32},
    {OXID_OWNED_RECORD, 0, 0, 2, 32},
    {OXID_OWNED_ENUM, 0, 0, 3, 32},
    {OXID_OWNED_ENUM, 0, 0, 3, 32},
    {OXID_OWNED_RECORD, 0, 0, 2, 32},
    {OXID_OWNED_ENUM, 0, 0, 3, 32},
    {OXID_OWNED_ENUM, 0, 0, 3, 32},
    {OXID_OWNED_ENUM, 0, 0, 3, 32},
    {OXID_OWNED_SCALAR, OXID_VECTOR_I128, 0, 0, 16},
    {OXID_OWNED_BYTEBUF, 0, 0, 0, 16}
};
const oxid_owned_field __oxid_owned_fields[] = {{8, 0}, {8, 16}};
const oxid_owned_field __oxid_owned_variants[] = {{UINT32_MAX, 0}, {9, 16}, {8, 16}};
const oxid_owned_record_schema __oxid_owned_records[] = {
    {0, 0, 32, 5, 0}, {1, 2, 32, 3, 1}
};
const oxid_owned_record_schema __oxid_owned_enums[] = {
    {ENUM_ROOT, 2, 32, 3, 1},
    {ENUM_ROOT | 1u, 2, 32, 3, 1}, /* wrong nominal enum root */
    {ENUM_ROOT | 2u, 4, 32, 3, 1}, /* record type in enum table */
    {ENUM_ROOT | 2u, 5, 32, 3, 1}, /* wrong storage key */
    {ENUM_ROOT | 4u, 6, 16, 3, 1}, /* wrong stride */
    {ENUM_ROOT | 5u, UINT32_MAX, 0, 0, 0}
};
const uint32_t __oxid_owned_type_count = 10, __oxid_owned_field_count = 2;
const uint32_t __oxid_owned_variant_count = 3, __oxid_owned_record_count = 2;
const uint32_t __oxid_owned_enum_count = 6;
#endif

static uint64_t enough(void) { return UINT64_C(1000000); }
static oxid_bytebuf incoming(oxid_byte_heap *heap, uint8_t bytes[32]) {
    oxid_bytebuf child = {0, 0, 0};
    uint64_t fuel = enough();
    assert(oxid_bytebuf_push(heap, &child, 42, &fuel) == OXID_GROW_OK);
    memset(bytes, 0xa5, 32);
    bytes[0] = 1; bytes[1] = bytes[2] = bytes[3] = 0;
    memcpy(bytes + 16, &child, 16);
    return child;
}
static void schema_domains_are_closed(void) {
    const oxid_owned_record_schema *schema = owned_record_schema(ENUM_ROOT);
    assert(schema && schema->record == ENUM_ROOT);
    assert(schema->root_type == __oxid_owned_record_count);
    assert(owned_record_type(schema->root_type)->kind == OXID_OWNED_ENUM);
    for (uint32_t id = 1; id <= 5; ++id) assert(!owned_record_schema(ENUM_ROOT | id));
    assert(!owned_record_schema(ENUM_ROOT | 4096u));
    assert(!owned_record_schema(UINT32_MAX));
    assert(!owned_record_schema(4096u));
    assert(owned_record_schema(0));
    assert(!owned_record_schema(1));
    oxid_owned_vector refused = {77, 3, 4}, before = refused;
    assert(oxid_owned_vector_new(ENUM_ROOT | 1u, &refused) == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(&refused, &before, sizeof(before)));
}
static void exact_move_and_cross_domain_refusal(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    oxid_owned_vector vector = {0, 0, 0};
    uint8_t source[32], original[32], output[32];
    oxid_bytebuf child = incoming(&heap, source);
    memcpy(original, source, 32);
    uint64_t fuel = 390;
    assert(oxid_owned_vector_reserve(&heap, &vector, ENUM_ROOT, 1, &fuel) == OXID_BUFFER_FUEL);
    assert(fuel == 390 && !vector.identity && !memcmp(source, original, 32));
    fuel = 391;
    assert(oxid_owned_vector_reserve(&heap, &vector, ENUM_ROOT, 1, &fuel) == OXID_GROW_OK && fuel == 0);
    assert(heap.ledger.live_bytes == 424);
    oxid_allocation *node;
    assert(locate_owned_vector(&heap, &vector, owned_record_schema(ENUM_ROOT), &node) == 0);
    assert((uintptr_t)owned_vector_data(node) % 16u == 0);
    assert(((oxid_owned_vector_header *)node)->record == ENUM_ROOT);
    fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &vector, 0, source, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(source, original, 32) && vector.length == 0);
    source[0] = 3;
    uint8_t poison[32]; memcpy(poison, source, 32);
    assert(oxid_owned_vector_push_reserved(&heap, &vector, ENUM_ROOT, source, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(source, poison, 32) && vector.length == 0);
    memcpy(source, original, 32);
    fuel = 125;
    assert(oxid_owned_vector_push_reserved(&heap, &vector, ENUM_ROOT, source, 32, &fuel) == OXID_BUFFER_FUEL);
    assert(fuel == 125 && !memcmp(source, original, 32));
    fuel = 126;
    assert(oxid_owned_vector_push_reserved(&heap, &vector, ENUM_ROOT, source, 32, &fuel) == 0 && fuel == 0);
    const uint8_t zero[32] = {0}; assert(!memcmp(source, zero, 32));
    memcpy(source, original, 32); fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &vector, ENUM_ROOT, source, 32, &fuel) == OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(source, original, 32) && vector.length == 1);
    assert(oxid_owned_vector_validate(&heap, &vector, 0, &fuel) == OXID_BUFFER_DESCRIPTOR);
    memset(output, 0xcc, 32); fuel = 93;
    assert(oxid_owned_vector_pop_last(&heap, &vector, ENUM_ROOT, output, 32, &fuel) == OXID_BUFFER_FUEL && fuel == 93);
    for (size_t i = 0; i < 32; ++i) assert(output[i] == 0xcc);
    fuel = 94;
    assert(oxid_owned_vector_pop_last(&heap, &vector, ENUM_ROOT, output, 32, &fuel) == 0 && fuel == 0);
    uint8_t expected[32] = {0}; expected[0] = 1; memcpy(expected + 16, &child, 16);
    assert(!memcmp(output, expected, 32));
    fuel = enough();
    assert(oxid_owned_vector_push_reserved(&heap, &vector, ENUM_ROOT, output, 32, &fuel) == 0);
    assert(oxid_owned_vector_release(&heap, &vector, ENUM_ROOT) == 0);
    assert(oxid_bytebuf_validate(&heap, &child) == OXID_BUFFER_DESCRIPTOR);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.allocations == heap.ledger.releases);
}
static void inactive_bytes_do_not_own_resources(void) {
    oxid_byte_heap heap; oxid_heap_init(&heap);
    uint8_t source[32], output[32];
    oxid_bytebuf child = incoming(&heap, source);
    source[0] = 0; /* The live child descriptor is in an inactive payload. */
    oxid_owned_vector vector = {0, 0, 0}; uint64_t fuel = enough();
    assert(oxid_owned_vector_reserve(&heap, &vector, ENUM_ROOT, 1, &fuel) == OXID_GROW_OK);
    assert(oxid_owned_vector_push_reserved(&heap, &vector, ENUM_ROOT, source, 32, &fuel) == 0);
    memset(output, 0xff, 32);
    assert(oxid_owned_vector_pop_last(&heap, &vector, ENUM_ROOT, output, 32, &fuel) == 0);
    const uint8_t zero[32] = {0}; assert(!memcmp(output, zero, 32));
    assert(oxid_owned_vector_release(&heap, &vector, ENUM_ROOT) == 0);
    assert(oxid_bytebuf_validate(&heap, &child) == 0);
    assert(oxid_bytebuf_release(&heap, &child) == 0);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.allocations == heap.ledger.releases);
}
int main(void) {
    schema_domains_are_closed();
    exact_move_and_cross_domain_refusal();
    inactive_bytes_do_not_own_resources();
    return 0;
}
