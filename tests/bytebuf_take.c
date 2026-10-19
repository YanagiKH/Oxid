/* Focused take adapter component checks; not emitted Oxid native acceptance. */
#include "../native/typed_bytebuf.c"
#include <assert.h>

static void transfer(int mode) {
    __oxid_bytebuf_start();
    uint64_t fuel = 1000;
    oxid_bytebuf source = {0, 0, 0}, result;
    const uint8_t payload[] = {65, 0, 194, 162, 240, 159, 152, 128};
    if (mode == 1)
        assert(oxid_bytebuf_reserve(&execution_heap, &source, 17, &fuel) == 0);
    if (mode == 2)
        assert(oxid_bytebuf_extend(&execution_heap, &source, payload, sizeof payload, &fuel) == 0);
    oxid_bytebuf original = source;
    oxid_heap_ledger ledger = execution_heap.ledger;
    oxid_allocation *head = execution_heap.head;
    uint64_t identity = execution_heap.next_identity, remaining = fuel;
    uint8_t *before = NULL, *after = NULL;
    assert(oxid_bytebuf_range(&execution_heap, &source, 0, source.length, &before) == 0);
    assert(__oxid_bytebuf_operation(10, &source, 0, 0, NULL, 0, &fuel, &result) == 0);
    assert(source.identity == 0 && source.length == 0 && source.capacity == 0);
    assert(memcmp(&original, &result, sizeof result) == 0);
    assert(memcmp(&ledger, &execution_heap.ledger, sizeof ledger) == 0);
    assert(execution_heap.head == head && execution_heap.next_identity == identity);
    assert(fuel == remaining);
    assert(oxid_bytebuf_range(&execution_heap, &result, 0, result.length, &after) == 0);
    assert(before == after);
    if (mode == 2) assert(memcmp(after, payload, sizeof payload) == 0);
    __oxid_bytebuf_drop(&source);
    assert(execution_heap.ledger.releases == ledger.releases);
    __oxid_bytebuf_drop(&result);
    assert(execution_heap.ledger.releases == ledger.releases + (mode != 0));
    assert(__oxid_bytebuf_finish() == 0);
}

int main(void) {
    transfer(0); transfer(1); transfer(2);
    __oxid_bytebuf_start();
    uint64_t fuel = 1000;
    oxid_bytebuf source = {0, 0, 0}, result = {42, 43, 44};
    assert(oxid_bytebuf_push(&execution_heap, &source, 65, &fuel) == 0);
    oxid_bytebuf original = source, sentinel = result;
    assert(__oxid_bytebuf_operation(10, &source, 0, 0, NULL, 0, &fuel, &source) == OXID_BUFFER_DESCRIPTOR);
    assert(memcmp(&original, &source, sizeof source) == 0);
    assert(__oxid_bytebuf_operation(10, &source, 0, 0, NULL, 0, &fuel,
        (uint8_t *)&source + 8) == OXID_BUFFER_DESCRIPTOR);
    assert(memcmp(&original, &source, sizeof source) == 0);
    oxid_bytebuf malformed = {0, 1, 0}, malformed_before = malformed;
    assert(__oxid_bytebuf_operation(10, &malformed, 0, 0, NULL, 0, &fuel, &result) == OXID_BUFFER_DESCRIPTOR);
    assert(memcmp(&malformed, &malformed_before, sizeof malformed) == 0);
    assert(memcmp(&result, &sentinel, sizeof result) == 0);
    assert(__oxid_bytebuf_operation(10, NULL, 0, 0, NULL, 0, &fuel, &result) == OXID_BUFFER_DESCRIPTOR);
    assert(memcmp(&result, &sentinel, sizeof result) == 0);
    assert(__oxid_bytebuf_operation(10, &source, 0, 0, NULL, 0, &fuel, &result) == 0);
    /* Model terminal failure after primitive commit, before owned return. */
    __oxid_bytebuf_cleanup();
    assert(execution_heap.ledger.live_allocations == 0);
    assert(execution_heap.ledger.releases == 1);
    return 0;
}
