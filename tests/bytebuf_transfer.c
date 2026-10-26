/* Descriptor-transfer controls; real native Oxid execution is checked separately. */
#ifndef OXID_BYTEBUF_TESTING
#define OXID_BYTEBUF_TESTING 1
#endif
#include "../native/typed_bytebuf.c"
#include <assert.h>

static void empty_and_populated(void) {
    __oxid_bytebuf_start();
    oxid_bytebuf value = {0}, result = {99, 99, 99};
    uint64_t fuel = 0;
    execution_heap.fault_boundary = ALLOCATE;
    assert(__oxid_bytebuf_operation(10, &value, 0, 0, NULL, 0, &fuel, &result) == 0);
    assert(result.identity == 0 && result.length == 0 && result.capacity == 0);
    assert(execution_heap.fault_boundary == ALLOCATE && fuel == 0);
    execution_heap.fault_boundary = 0;
    const uint8_t bytes[] = {0, 255, 195, 169};
    fuel = 100;
    assert(oxid_bytebuf_extend(&execution_heap, &value, bytes, sizeof bytes, &fuel) == OXID_GROW_OK);
    oxid_bytebuf before = value;
    oxid_heap_ledger ledger = execution_heap.ledger;
    uint64_t identity = execution_heap.next_identity;
    execution_heap.fault_boundary = ALLOCATE;
    fuel = 0;
    assert(__oxid_bytebuf_operation(10, &value, 0, 0, NULL, 0, &fuel, &result) == 0);
    assert(memcmp(&result, &before, sizeof before) == 0);
    assert(value.identity == 0 && value.length == 0 && value.capacity == 0);
    assert(memcmp(&ledger, &execution_heap.ledger, sizeof ledger) == 0);
    assert(execution_heap.next_identity == identity && execution_heap.fault_boundary == ALLOCATE && fuel == 0);
    uint8_t *view;
    assert(oxid_bytebuf_range(&execution_heap, &result, 0, result.length, &view) == 0);
    assert(memcmp(view, bytes, sizeof bytes) == 0);
    fuel = 100;
    assert(oxid_bytebuf_push(&execution_heap, &value, 77, &fuel) == OXID_ALLOCATION_FAILED);
    assert(value.identity == 0 && execution_heap.fault_boundary == 0);
    assert(oxid_bytebuf_push(&execution_heap, &value, 77, &fuel) == OXID_GROW_OK);
    assert(result.identity == before.identity && result.length == 4);
    __oxid_bytebuf_drop(&value);
    __oxid_bytebuf_drop(&result);
    assert(execution_heap.ledger.live_allocations == 0);
    assert(execution_heap.ledger.allocations == execution_heap.ledger.releases);
    assert(__oxid_bytebuf_finish() == 0);
}
static void invalid_and_overlapping(void) {
    __oxid_bytebuf_start();
    oxid_bytebuf value = {0}, output = {77, 88, 99};
    uint64_t fuel = 100;
    assert(oxid_bytebuf_push(&execution_heap, &value, 255, &fuel) == OXID_GROW_OK);
    oxid_bytebuf before = value, unchanged_output = output;
    oxid_heap_ledger ledger = execution_heap.ledger;
    assert(__oxid_bytebuf_operation(10, NULL, 0, 0, NULL, 0, &fuel, &output) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_bytebuf_operation(10, &value, 0, 0, NULL, 0, NULL, &output) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_bytebuf_operation(10, &value, 0, 0, NULL, 0, &fuel, NULL) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_bytebuf_operation(10, &value, 0, 0, NULL, 0, &fuel, &value) == OXID_BUFFER_DESCRIPTOR);
    struct { oxid_bytebuf value; uint8_t tail[16]; } after = {before, {0}};
    assert(__oxid_bytebuf_operation(10, &after.value, 0, 0, NULL, 0, &fuel, (uint8_t *)&after.value + 8) == OXID_BUFFER_DESCRIPTOR);
    struct { uint8_t prefix[8]; oxid_bytebuf value; } prior = {{0}, before};
    assert(__oxid_bytebuf_operation(10, &prior.value, 0, 0, NULL, 0, &fuel, prior.prefix) == OXID_BUFFER_DESCRIPTOR);
    assert(memcmp(&after.value, &before, sizeof before) == 0 && memcmp(&prior.value, &before, sizeof before) == 0);
    oxid_bytebuf invalid = before;
    invalid.capacity += 1;
    oxid_bytebuf invalid_before = invalid;
    assert(__oxid_bytebuf_operation(10, &invalid, 0, 0, NULL, 0, &fuel, &output) == OXID_BUFFER_DESCRIPTOR);
    assert(memcmp(&invalid, &invalid_before, sizeof invalid) == 0);
    assert(memcmp(&value, &before, sizeof before) == 0 && memcmp(&output, &unchanged_output, sizeof output) == 0);
    assert(memcmp(&ledger, &execution_heap.ledger, sizeof ledger) == 0);
    __oxid_bytebuf_drop(&value);
    assert(__oxid_bytebuf_finish() == 0);
}
static void full_heap(void) {
    __oxid_bytebuf_start();
    oxid_bytebuf original = {0}, filler = {0}, result = {0};
    uint64_t fuel = 100;
    assert(oxid_bytebuf_resize(&execution_heap, &original, 8, 255, &fuel) == OXID_GROW_OK);
    assert(oxid_bytebuf_reserve(&execution_heap, &filler, 16777144, &fuel) == OXID_GROW_OK);
    assert(execution_heap.ledger.live_bytes == OXID_BYTEBUF_LIMIT);
    oxid_heap_ledger before = execution_heap.ledger;
    uint64_t identity = execution_heap.next_identity;
    fuel = 0;
    assert(__oxid_bytebuf_operation_hosted(10, &original, 0, 0, NULL, 0, &fuel, &result) == 0);
    assert(original.identity == 0 && result.length == 8 && result.capacity == 8 && fuel == 0);
    assert(memcmp(&before, &execution_heap.ledger, sizeof before) == 0 && execution_heap.next_identity == identity);
    __oxid_bytebuf_drop(&filler);
    __oxid_bytebuf_drop(&original);
    __oxid_bytebuf_drop(&result);
    assert(__oxid_bytebuf_finish() == 0);
}
int main(void) { empty_and_populated(); invalid_and_overlapping(); full_heap(); return 0; }
