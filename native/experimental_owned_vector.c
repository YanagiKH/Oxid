/* Included after experimental_bytebuf.c and its scalar/Copy vector core.
 * This distinct class keeps the existing registry prefix and old layouts.
 * Runtime operation/catalog adapters are defined separately before admission. */
#include "experimental_owned_vector.h"
#include <limits.h>

typedef struct {
    _Alignas(16) oxid_allocation prefix;
    uint32_t capacity, record;
    uint64_t stride, allocation_bytes, identity_offset;
} oxid_owned_vector_header;

_Static_assert(sizeof(oxid_owned_vector) == 16, "owned vector descriptor bytes");
_Static_assert(_Alignof(oxid_owned_vector) == 8, "owned vector descriptor alignment");
_Static_assert(sizeof(oxid_allocation) == 32, "unchanged registry prefix");
_Static_assert(sizeof(oxid_owned_vector_header) == 64, "owned vector registry bytes");
_Static_assert(_Alignof(oxid_owned_vector_header) == 16, "owned vector registry alignment");
_Static_assert(offsetof(oxid_owned_vector_header, prefix) == 0, "owned vector intrusive prefix");
_Static_assert(offsetof(oxid_owned_vector_header, capacity) == 32, "owned vector capacity offset");
_Static_assert(offsetof(oxid_owned_vector_header, record) == 36, "owned vector nominal offset");
_Static_assert(offsetof(oxid_owned_vector_header, stride) == 40, "owned vector stride offset");
_Static_assert(offsetof(oxid_owned_vector_header, allocation_bytes) == 48, "owned vector charge offset");
_Static_assert(offsetof(oxid_owned_vector_header, identity_offset) == 56, "owned vector tail offset");

uint64_t oxid_owned_vector_node_charge(const oxid_allocation *node) {
    return ((const oxid_owned_vector_header *)node)->allocation_bytes;
}

/* Closed checked type facts only. A successful layout is not a descriptor,
 * source type, executable witness or admission for an arbitrary caller. */
static int owned_vector_layout(uint32_t stride, int32_t capacity,
    uint32_t maximum_resource_leaves, size_t *identity_offset,
    size_t *identity_slots, size_t *allocation_bytes) {
    if (!stride || capacity < 0 || maximum_resource_leaves > stride / 16u)
        return 0;
    if ((size_t)capacity > (SIZE_MAX - 64u - 7u) / stride) return 0;
    size_t tail = (64u + (size_t)capacity * stride + 7u) & ~(size_t)7u;
    if (maximum_resource_leaves &&
        (size_t)capacity > SIZE_MAX / maximum_resource_leaves) return 0;
    size_t slots = (size_t)capacity * maximum_resource_leaves;
    if (tail > SIZE_MAX - 15u || slots > (SIZE_MAX - tail - 15u) / sizeof(uint64_t))
        return 0;
    size_t bytes = (tail + slots * sizeof(uint64_t) + 15u) & ~(size_t)15u;
    if (bytes > PTRDIFF_MAX) return 0;
    *identity_offset = tail;
    *identity_slots = slots;
    *allocation_bytes = bytes;
    return 1;
}

#ifdef OXID_OWNED_VECTOR_RUNTIME
#include "experimental_owned_record.c"

#ifndef OXID_OWNED_VECTOR_ALIGNED_ALLOC
#define OXID_OWNED_VECTOR_ALIGNED_ALLOC aligned_alloc
#endif
typedef struct { const void *pointer; size_t bytes; } owned_vector_range;
static int owned_vector_add(uint64_t a, uint64_t b, uint64_t *out) {
    if (a > UINT64_MAX - b) return 0;
    *out = a + b;
    return 1;
}
static int owned_vector_mul(uint64_t a, uint64_t b, uint64_t *out) {
    if (b && a > UINT64_MAX / b) return 0;
    *out = a * b;
    return 1;
}
static uint8_t *owned_vector_data(oxid_allocation *node) {
    return (uint8_t *)node + OXID_OWNED_VECTOR_HEADER;
}
static uint64_t owned_vector_extent(const oxid_allocation *node) {
    if (node->capacity == OXID_OWNED_VECTOR_NODE) return oxid_owned_vector_node_charge(node);
    if (node->capacity == OXID_VECTOR_NODE) return oxid_vector_node_charge(node);
    return OXID_BYTEBUF_HEADER + (node->capacity == UINT32_MAX ? 0 : (uint64_t)node->capacity);
}
static int owned_vector_disjoint(oxid_byte_heap *heap,
    const owned_vector_range *ranges, size_t count) {
    for (size_t i = 0; i < count; ++i) {
        uintptr_t start = (uintptr_t)ranges[i].pointer;
        if ((ranges[i].bytes && !start) || ranges[i].bytes > UINTPTR_MAX - start ||
            vector_overlaps(ranges[i].pointer, ranges[i].bytes, heap, sizeof(*heap))) return 0;
        for (size_t j = 0; j < i; ++j)
            if (vector_overlaps(ranges[i].pointer, ranges[i].bytes,
                ranges[j].pointer, ranges[j].bytes)) return 0;
        for (oxid_allocation *node = heap->head; node; node = node->next) {
            uint64_t bytes = owned_vector_extent(node);
            if (bytes > SIZE_MAX || bytes > UINTPTR_MAX - (uintptr_t)node ||
                vector_overlaps(ranges[i].pointer, ranges[i].bytes, node, (size_t)bytes)) return 0;
        }
    }
    return 1;
}
static int owned_vector_controls(oxid_byte_heap *heap,
    const owned_vector_range *ranges, size_t count, uint64_t fuel, uint64_t *remaining) {
    uint64_t work, scans, pairs;
    if (!owned_vector_mul(heap->ledger.live_allocations, count + 1u, &scans) ||
        !owned_vector_mul(count, count ? count - 1u : 0, &pairs) ||
        !owned_vector_add(1u + count, pairs / 2u, &work) ||
        !owned_vector_add(work, scans, &work) || fuel < work) return OXID_BUFFER_FUEL;
    if (!owned_vector_disjoint(heap, ranges, count)) return OXID_BUFFER_DESCRIPTOR;
    *remaining = fuel - work;
    return 0;
}
static int32_t locate_owned_vector(oxid_byte_heap *heap, const oxid_owned_vector *value,
    const oxid_owned_record_schema *schema, oxid_allocation **out) {
    *out = NULL;
    if (!schema) return OXID_BUFFER_DESCRIPTOR;
    if (!value->identity)
        return !value->length && !value->capacity ? 0 : OXID_BUFFER_DESCRIPTOR;
    if (value->length < 0 || value->capacity <= 0 || value->length > value->capacity)
        return OXID_BUFFER_DESCRIPTOR;
    for (oxid_allocation *node = heap->head; node; node = node->next) {
        if (node->identity != value->identity) continue;
        if (node->capacity != OXID_OWNED_VECTOR_NODE || (uintptr_t)node % 16u)
            return OXID_BUFFER_DESCRIPTOR;
        const oxid_owned_vector_header *header = (const oxid_owned_vector_header *)node;
        size_t offset, slots, bytes;
        if (!owned_vector_layout((uint32_t)schema->stride, value->capacity,
                (uint32_t)schema->maximum_resource_leaves, &offset, &slots, &bytes) ||
            header->record != schema->record || header->stride != schema->stride ||
            header->capacity != (uint32_t)value->capacity ||
            node->initialized != (uint32_t)value->length ||
            header->allocation_bytes != bytes || header->identity_offset != offset)
            return OXID_BUFFER_DESCRIPTOR;
        *out = node;
        return 0;
    }
    return OXID_BUFFER_DESCRIPTOR;
}
static int owned_vector_work(oxid_byte_heap *heap,
    const oxid_owned_record_schema *schema, uint64_t records, uint64_t copied,
    uint64_t *work) {
    uint64_t children, search, sort, visits, bits = 0;
    if (!owned_vector_mul(schema->maximum_resource_leaves, records, &children) ||
        !owned_vector_mul(children, heap->ledger.live_allocations, &search)) return 0;
    for (uint64_t value = children; value; value >>= 1u) ++bits;
    return owned_vector_mul(children, bits, &sort) && owned_vector_mul(sort, 4, &sort) &&
        owned_vector_mul(schema->visits, records, &visits) &&
        owned_vector_mul(visits, 3, &visits) && owned_vector_add(visits, search, work) &&
        owned_vector_add(*work, sort, work) && owned_vector_add(*work, copied, work);
}
int32_t oxid_owned_vector_validate(oxid_byte_heap *heap, const oxid_owned_vector *value,
    uint32_t record, uint64_t *fuel) {
    if (!heap || !value || !fuel) return OXID_BUFFER_DESCRIPTOR;
    const owned_vector_range ranges[] = {{value, sizeof(*value)}, {fuel, sizeof(*fuel)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 2, *fuel, &remaining);
    if (status) return status;
    oxid_allocation *node;
    status = locate_owned_vector(heap, value, owned_record_schema(record), &node);
    if (status) return status;
    *fuel = remaining;
    return 0;
}
int32_t oxid_owned_vector_validate_maintenance(oxid_byte_heap *heap,
    const oxid_owned_vector *value, uint32_t record) {
    if (!heap || !value) return OXID_BUFFER_DESCRIPTOR;
    const owned_vector_range range = {value, sizeof(*value)};
    oxid_allocation *node;
    int32_t status = locate_owned_vector(heap, value, owned_record_schema(record), &node);
    if (status) return status;
    return owned_vector_disjoint(heap, &range, 1) ? 0 : OXID_BUFFER_DESCRIPTOR;
}
static int owned_vector_preflight(oxid_byte_heap *heap, oxid_allocation *node,
    int32_t length, const oxid_owned_record_schema *schema, const uint8_t *incoming) {
    uint64_t *identities = NULL;
    size_t capacity = 0, used = 0;
    if (node) {
        const oxid_owned_vector_header *header = (const oxid_owned_vector_header *)node;
        size_t offset, bytes;
        if (!owned_vector_layout((uint32_t)schema->stride, (int32_t)header->capacity,
            (uint32_t)schema->maximum_resource_leaves, &offset, &capacity, &bytes)) return 0;
        identities = (uint64_t *)((uint8_t *)node + offset);
        for (int32_t index = 0; index < length; ++index)
            if (!owned_record_check(heap, schema,
                owned_vector_data(node) + (size_t)index * schema->stride,
                (size_t)schema->stride, identities, capacity, &used)) return 0;
    }
    if (incoming && !owned_record_check(heap, schema, incoming, (size_t)schema->stride,
            identities, capacity, &used)) return 0;
    return owned_record_unique(identities, used);
}
int32_t oxid_owned_vector_new(uint32_t record, oxid_owned_vector *result) {
    if (!result || !owned_record_schema(record)) return OXID_BUFFER_DESCRIPTOR;
    *result = (oxid_owned_vector){0, 0, 0};
    return 0;
}
int32_t oxid_owned_vector_reserve(oxid_byte_heap *heap, oxid_owned_vector *value,
    uint32_t record, int32_t additional, uint64_t *fuel) {
    if (!heap || !value || !fuel) return OXID_BUFFER_DESCRIPTOR;
    const oxid_owned_record_schema *schema = owned_record_schema(record);
    owned_vector_range ranges[] = {{value, sizeof(*value)}, {fuel, sizeof(*fuel)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 2, *fuel, &remaining);
    if (status) return status;
    oxid_allocation *old;
    status = locate_owned_vector(heap, value, schema, &old);
    if (status) return status;
    if (additional < 0) return OXID_BUFFER_BOUNDS;
    if (additional > INT32_MAX - value->length) { *fuel = remaining; return OXID_CAPACITY_OVERFLOW; }
    int32_t capacity = oxid_bytebuf_growth(value->capacity, value->length + additional);
    if (capacity == value->capacity) { *fuel = remaining; return OXID_GROW_OK; }
    size_t offset, slots, bytes;
    if (!owned_vector_layout((uint32_t)schema->stride, capacity,
        (uint32_t)schema->maximum_resource_leaves, &offset, &slots, &bytes)) {
        *fuel = remaining; return OXID_CAPACITY_OVERFLOW;
    }
    uint64_t copied, writes, work;
    if (!owned_vector_mul((uint64_t)value->length, schema->stride, &copied) ||
        !owned_vector_add(bytes, copied, &writes) ||
        !owned_vector_work(heap, schema, (uint64_t)value->length, writes, &work) ||
        remaining < work) return OXID_BUFFER_FUEL;
    if (!owned_vector_preflight(heap, old, value->length, schema, NULL)) return OXID_BUFFER_DESCRIPTOR;
    *fuel = remaining - work;
    if (bytes > heap->limit || heap->ledger.live_bytes > heap->limit - bytes) return OXID_MEMORY_LIMIT;
    if (heap->next_identity == UINT64_MAX) return OXID_BUFFER_IDENTITY;
    if (fail(heap, ALLOCATE)) return OXID_ALLOCATION_FAILED;
    oxid_owned_vector_header *target = OXID_OWNED_VECTOR_ALIGNED_ALLOC(16, bytes);
    if (!target) return OXID_ALLOCATION_FAILED;
    *target = (oxid_owned_vector_header){
        {heap->head, NULL, heap->next_identity++, OXID_OWNED_VECTOR_NODE, 0},
        (uint32_t)capacity, record, schema->stride, bytes, offset
    };
    if (heap->head) heap->head->previous = &target->prefix;
    heap->head = &target->prefix;
    heap->ledger.live_bytes += bytes;
    ++heap->ledger.live_allocations; ++heap->ledger.allocations;
    if (heap->ledger.live_bytes > heap->ledger.peak_bytes) heap->ledger.peak_bytes = heap->ledger.live_bytes;
    if (fail(heap, INITIALIZE)) { unlink_node(heap, &target->prefix); return OXID_ALLOCATION_FAILED; }
    memset(owned_vector_data(&target->prefix), 0, bytes - 64u);
    if (fail(heap, COMMIT)) { unlink_node(heap, &target->prefix); return OXID_ALLOCATION_FAILED; }
    if (old && copied) memcpy(owned_vector_data(&target->prefix), owned_vector_data(old), (size_t)copied);
    target->prefix.initialized = (uint32_t)value->length;
    value->identity = target->prefix.identity;
    value->capacity = capacity;
    if (old) unlink_node(heap, old);
    return OXID_GROW_OK;
}
static int32_t owned_vector_query(oxid_byte_heap *heap, const oxid_owned_vector *value,
    uint32_t record, uint64_t *fuel, int32_t *result, int capacity) {
    if (!heap || !value || !fuel || !result) return OXID_BUFFER_DESCRIPTOR;
    owned_vector_range ranges[] = {{value, sizeof(*value)}, {fuel, sizeof(*fuel)}, {result, sizeof(*result)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 3, *fuel, &remaining);
    if (status) return status;
    if (remaining < sizeof(*result)) return OXID_BUFFER_FUEL;
    oxid_allocation *node;
    status = locate_owned_vector(heap, value, owned_record_schema(record), &node);
    if (status) return status;
    *fuel = remaining - sizeof(*result);
    *result = capacity ? value->capacity : value->length;
    return 0;
}
int32_t oxid_owned_vector_len(oxid_byte_heap *heap, const oxid_owned_vector *value,
    uint32_t record, uint64_t *fuel, int32_t *result) {
    return owned_vector_query(heap, value, record, fuel, result, 0);
}
int32_t oxid_owned_vector_capacity(oxid_byte_heap *heap, const oxid_owned_vector *value,
    uint32_t record, uint64_t *fuel, int32_t *result) {
    return owned_vector_query(heap, value, record, fuel, result, 1);
}
int32_t oxid_owned_vector_push_reserved(oxid_byte_heap *heap, oxid_owned_vector *value,
    uint32_t record, uint8_t *source, size_t bytes, uint64_t *fuel) {
    if (!heap || !value || !fuel) return OXID_BUFFER_DESCRIPTOR;
    owned_vector_range ranges[] = {{value, sizeof(*value)}, {source, bytes}, {fuel, sizeof(*fuel)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 3, *fuel, &remaining);
    if (status) return status;
    const oxid_owned_record_schema *schema = owned_record_schema(record);
    oxid_allocation *node;
    status = locate_owned_vector(heap, value, schema, &node);
    if (status) return status;
    if (value->length == value->capacity) return OXID_BUFFER_BOUNDS;
    if (bytes != schema->stride) return OXID_BUFFER_DESCRIPTOR;
    uint64_t writes, work;
    if (!owned_vector_mul(schema->stride, 3, &writes) ||
        !owned_vector_work(heap, schema, (uint64_t)value->length + 1u, writes, &work) ||
        remaining < work) return OXID_BUFFER_FUEL;
    if (!owned_vector_preflight(heap, node, value->length, schema, source)) return OXID_BUFFER_DESCRIPTOR;
    if (!owned_record_copy(schema, source, owned_vector_data(node) +
        (size_t)value->length * bytes, bytes)) return OXID_BUFFER_DESCRIPTOR;
    *fuel = remaining - work;
    memset(source, 0, bytes);
    ++value->length; ++node->initialized;
    return 0;
}
int32_t oxid_owned_vector_pop_last(oxid_byte_heap *heap, oxid_owned_vector *value,
    uint32_t record, uint8_t *destination, size_t bytes, uint64_t *fuel) {
    if (!heap || !value || !fuel) return OXID_BUFFER_DESCRIPTOR;
    owned_vector_range ranges[] = {{value, sizeof(*value)}, {destination, bytes}, {fuel, sizeof(*fuel)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 3, *fuel, &remaining);
    if (status) return status;
    const oxid_owned_record_schema *schema = owned_record_schema(record);
    oxid_allocation *node;
    status = locate_owned_vector(heap, value, schema, &node);
    if (status) return status;
    if (!value->length) return OXID_BUFFER_BOUNDS;
    if (bytes != schema->stride) return OXID_BUFFER_DESCRIPTOR;
    uint64_t writes, work;
    if (!owned_vector_mul(schema->stride, 2, &writes) ||
        !owned_vector_work(heap, schema, (uint64_t)value->length, writes, &work) ||
        remaining < work) return OXID_BUFFER_FUEL;
    if (!owned_vector_preflight(heap, node, value->length, schema, NULL)) return OXID_BUFFER_DESCRIPTOR;
    uint8_t *source = owned_vector_data(node) + (size_t)(value->length - 1) * bytes;
    memcpy(destination, source, bytes);
    memset(source, 0, bytes);
    *fuel = remaining - work;
    --value->length; --node->initialized;
    return 0;
}
static void owned_vector_commit_truncate(oxid_byte_heap *heap, oxid_allocation *node,
    oxid_owned_vector *value, const oxid_owned_record_schema *schema, int32_t length) {
    if (node) {
        for (int32_t index = length; index < value->length; ++index)
            owned_record_release(heap, schema,
                owned_vector_data(node) + (size_t)index * schema->stride, (size_t)schema->stride);
        memset(owned_vector_data(node) + (size_t)length * schema->stride, 0,
            (size_t)(value->length - length) * schema->stride);
        node->initialized = (uint32_t)length;
    }
    value->length = length;
}
int32_t oxid_owned_vector_truncate(oxid_byte_heap *heap, oxid_owned_vector *value,
    uint32_t record, int32_t length, uint64_t *fuel) {
    if (!heap || !value || !fuel) return OXID_BUFFER_DESCRIPTOR;
    owned_vector_range ranges[] = {{value, sizeof(*value)}, {fuel, sizeof(*fuel)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 2, *fuel, &remaining);
    if (status) return status;
    const oxid_owned_record_schema *schema = owned_record_schema(record);
    oxid_allocation *node;
    status = locate_owned_vector(heap, value, schema, &node);
    if (status) return status;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if (length >= value->length) { *fuel = remaining; return 0; }
    uint64_t removed = (uint64_t)(value->length - length), writes, release, work;
    if (!owned_vector_mul(removed, schema->stride, &writes) ||
        !owned_vector_mul(removed, schema->maximum_resource_leaves, &release) ||
        !owned_vector_mul(release, 2, &release) ||
        !owned_vector_mul(release, heap->ledger.live_allocations, &release) ||
        !owned_vector_work(heap, schema, (uint64_t)value->length, writes, &work) ||
        !owned_vector_add(work, release, &work) || remaining < work) return OXID_BUFFER_FUEL;
    if (!owned_vector_preflight(heap, node, value->length, schema, NULL)) return OXID_BUFFER_DESCRIPTOR;
    *fuel = remaining - work;
    owned_vector_commit_truncate(heap, node, value, schema, length);
    return 0;
}
int32_t oxid_owned_vector_take(oxid_byte_heap *heap, oxid_owned_vector *value,
    uint32_t record, uint64_t *fuel, oxid_owned_vector *result) {
    if (!heap || !value || !fuel || !result) return OXID_BUFFER_DESCRIPTOR;
    owned_vector_range ranges[] = {{value, sizeof(*value)}, {fuel, sizeof(*fuel)}, {result, sizeof(*result)}};
    uint64_t remaining;
    int32_t status = owned_vector_controls(heap, ranges, 3, *fuel, &remaining);
    if (status) return status;
    if (remaining < 2u * sizeof(*value)) return OXID_BUFFER_FUEL;
    oxid_allocation *node;
    status = locate_owned_vector(heap, value, owned_record_schema(record), &node);
    if (status) return status;
    *result = *value;
    *value = (oxid_owned_vector){0, 0, 0};
    *fuel = remaining - 2u * sizeof(*value);
    return 0;
}
int32_t oxid_owned_vector_release(oxid_byte_heap *heap, oxid_owned_vector *value,
    uint32_t record) {
    if (!heap || !value) return OXID_BUFFER_DESCRIPTOR;
    const oxid_owned_record_schema *schema = owned_record_schema(record);
    owned_vector_range range = {value, sizeof(*value)};
    oxid_allocation *node;
    int32_t status = locate_owned_vector(heap, value, schema, &node);
    if (status) return status;
    if (!owned_vector_disjoint(heap, &range, 1) ||
        !owned_vector_preflight(heap, node, value->length, schema, NULL)) return OXID_BUFFER_DESCRIPTOR;
    owned_vector_commit_truncate(heap, node, value, schema, 0);
    if (node) unlink_node(heap, node);
    *value = (oxid_owned_vector){0, 0, 0};
    return 0;
}
#endif
