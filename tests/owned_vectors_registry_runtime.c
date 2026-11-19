/* Registry component controls. These do not admit source-language operations. */
#define _POSIX_C_SOURCE 200809L
#define OXID_OWNED_VECTOR_RUNTIME
#define OXID_RECORD_VECTOR_SCHEMAS
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "../native/experimental_bytebuf.c"
#include "../native/experimental_owned_vector.c"
static oxid_byte_heap execution_heap;
#include "../native/typed_vector.c"

static const oxid_vector_record_leaf leaves[] = {{0, 0, OXID_VECTOR_U32}};
const uint32_t __oxid_record_vector_schema_count = 1;
const oxid_vector_record_schema __oxid_record_vector_schemas[] = {
    {OXID_VECTOR_RECORD_BIT, 4, 8, 1, leaves}
};

/* Only this fixture constructs registry nodes directly. Cleanup must use their
 * class and stored allocation charge, never payload bits or a descriptor. */
static void register_node(oxid_byte_heap *heap, oxid_allocation *node,
    uint32_t kind, uint32_t initialized, uint64_t bytes) {
    *node = (oxid_allocation){heap->head, NULL, heap->next_identity++, kind, initialized};
    if (heap->head) heap->head->previous = node;
    heap->head = node;
    heap->ledger.live_bytes += bytes;
    ++heap->ledger.live_allocations;
    ++heap->ledger.allocations;
    if (heap->ledger.live_bytes > heap->ledger.peak_bytes)
        heap->ledger.peak_bytes = heap->ledger.live_bytes;
}
static oxid_owned_vector_header *owned_fixture(oxid_byte_heap *heap) {
    size_t offset, slots, bytes;
    assert(owned_vector_layout(24, 1, 1, &offset, &slots, &bytes));
    assert(offset == 88 && slots == 1 && bytes == 96);
    oxid_owned_vector_header *header = aligned_alloc(16, 96);
    assert(header && (uintptr_t)header % 16u == 0);
    memset(header, 0xff, 96);
    register_node(heap, &header->prefix, OXID_OWNED_VECTOR_NODE, 1, 96);
    header->capacity = 1;
    header->record = 3;
    header->stride = 24;
    header->allocation_bytes = 96;
    header->identity_offset = 88;
    return header;
}
static void owned_cleanup_uses_exact_charge(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    owned_fixture(&heap);
    assert(heap.ledger.live_bytes == 96 && heap.ledger.live_allocations == 1);
    oxid_heap_cleanup(&heap);
    assert(heap.head == NULL && heap.ledger.live_bytes == 0);
    assert(heap.ledger.live_allocations == 0 && heap.ledger.allocations == 1);
    assert(heap.ledger.releases == 1);
    oxid_heap_cleanup(&heap);
    assert(heap.ledger.releases == 1);
}
static void old_record_source_uses_the_owned_extent(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_owned_vector_header *header = owned_fixture(&heap);
    const uint8_t *first = (const uint8_t *)header;
    /* All tests use numeric intersection only; these ranges are never read. */
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, first, 4));
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, first + 64, 4));
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, first + 92, 4));
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, first + 95, 4));
    assert(vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, first + 96, 4));
    assert(vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, first + 96, 0));
    assert(heap.ledger.live_bytes == 96 && heap.ledger.releases == 0);
    oxid_heap_cleanup(&heap);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.releases == 1);
}
static void mixed_registry_cleanup_is_flat_and_closes_files(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    uint64_t fuel = 1000000;
    oxid_bytebuf bytes = {0, 0, 0};
    oxid_vector scalar = {0, 0, 0}, record = {0, 0, 0};
    const uint8_t u32[] = {1, 2, 3, 4};
    assert(oxid_bytebuf_push(&heap, &bytes, 7, &fuel) == OXID_GROW_OK);
    assert(heap.ledger.live_bytes == 40);
    owned_fixture(&heap);
    assert(oxid_vector_push(&heap, &scalar, OXID_VECTOR_U32, u32, &fuel) == OXID_GROW_OK);
    assert(oxid_vector_push(&heap, &record, OXID_VECTOR_RECORD_BIT, u32, &fuel) == OXID_GROW_OK);
    int fd = open("/dev/null", O_RDONLY);
    assert(fd >= 0 && (uint64_t)fd < UINT32_MAX);
    oxid_allocation *file = malloc(32);
    assert(file);
    register_node(&heap, file, UINT32_MAX, (uint32_t)fd, 32);
    owned_fixture(&heap);
    assert(heap.ledger.live_bytes == 424 && heap.ledger.live_allocations == 6);
    assert(fcntl(fd, F_GETFD) >= 0);
    /* The owned payload and identity tail contain invalid all-one bits.
     * Fatal cleanup must not decode them or recursively release their owners. */
    heap.limit = heap.ledger.live_bytes;
    oxid_heap_cleanup(&heap);
    assert(heap.head == NULL && heap.ledger.live_bytes == 0);
    assert(heap.ledger.live_allocations == 0);
    assert(heap.ledger.allocations == 6 && heap.ledger.releases == 6);
    errno = 0;
    assert(fcntl(fd, F_GETFD) == -1 && errno == EBADF);
}
static void old_record_vector_ranges_stay_nominal(void) {
    oxid_byte_heap heap;
    oxid_heap_init(&heap);
    oxid_vector record = {0, 0, 0};
    uint64_t fuel = 1000000;
    const uint8_t u32[] = {9, 8, 7, 6};
    assert(oxid_vector_push(&heap, &record, OXID_VECTOR_RECORD_BIT, u32, &fuel) == OXID_GROW_OK);
    uint8_t *dense = NULL;
    assert(oxid_vector_range(&heap, &record, OXID_VECTOR_RECORD_BIT, 0, 1, &dense) == 0);
    owned_fixture(&heap);
    assert(vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, dense, 4));
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, dense + 1, 4));
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT, dense + 4, 4));
    assert(!vector_record_source(&heap, OXID_VECTOR_RECORD_BIT | 1u, dense, 4));
    assert(!memcmp(dense, u32, 4));
    oxid_heap_cleanup(&heap);
    assert(heap.ledger.live_bytes == 0 && heap.ledger.releases == 2);
}
static void generated_old_adapter_uses_exact_owned_extents(void) {
    oxid_heap_init(&execution_heap);
    oxid_owned_vector_header *node = owned_fixture(&execution_heap);
    const uint8_t *first = (const uint8_t *)node;
    assert(vector_heap_overlap(first, 4));
    assert(vector_heap_overlap(first + 64, 4));
    assert(vector_heap_overlap(first + 95, 4));
    assert(!vector_heap_overlap(first + 96, 4));
    uint64_t fuel = 1000000;
    oxid_vector result = {71, 2, 3};
    assert(__oxid_vector_operation(0, OXID_VECTOR_U32, NULL, 0,
        NULL, NULL, 0, &fuel, &result, 0) == 0);
    assert(result.identity == 0 && result.length == 0 && result.capacity == 0);
    oxid_heap_cleanup(&execution_heap);
    assert(execution_heap.ledger.live_bytes == 0 && execution_heap.ledger.releases == 1);
}
int main(void) {
    owned_cleanup_uses_exact_charge();
    old_record_source_uses_the_owned_extent();
    mixed_registry_cleanup_is_flat_and_closes_files();
    old_record_vector_ranges_stay_nominal();
    generated_old_adapter_uses_exact_owned_extents();
    puts("owned vector registry components: 5 passed");
    return 0;
}
