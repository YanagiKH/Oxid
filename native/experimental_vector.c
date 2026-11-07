/* Included after experimental_bytebuf.c in the same translation unit. The
 * existing registry's unlink_node must dispatch vector-class accounting; see
 * INTEGRATION.md. Header metadata is private, never supplied by source bytes. */
#include "experimental_vector.h"
#include <limits.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    _Alignas(16) oxid_allocation prefix;
    uint32_t capacity, element;
    uint64_t allocation_bytes;
} oxid_vector_header;
_Static_assert(sizeof(oxid_vector) == 16, "vector descriptor bytes");
_Static_assert(_Alignof(oxid_vector) == 8, "vector descriptor alignment");
_Static_assert(sizeof(oxid_allocation) == 32, "unchanged registry prefix");
_Static_assert(sizeof(oxid_vector_header) == 48, "vector registry bytes");
_Static_assert(_Alignof(oxid_vector_header) == 16, "vector registry alignment");
_Static_assert(offsetof(oxid_vector_header, prefix) == 0, "vector intrusive prefix");
_Static_assert(offsetof(oxid_vector_header, capacity) == 32, "vector metadata offset");

#ifndef OXID_VECTOR_ALIGNED_ALLOC
#define OXID_VECTOR_ALIGNED_ALLOC aligned_alloc
#endif

static size_t vector_scalar_size(uint32_t element) {
    switch (element) {
    case OXID_VECTOR_BOOL: case OXID_VECTOR_UNIT:
    case OXID_VECTOR_I8: case OXID_VECTOR_U8: return 1;
    case OXID_VECTOR_I16: case OXID_VECTOR_U16: return 2;
    case OXID_VECTOR_I32: case OXID_VECTOR_U32:
    case OXID_VECTOR_F32: case OXID_VECTOR_CHAR: return 4;
    case OXID_VECTOR_I64: case OXID_VECTOR_U64:
    case OXID_VECTOR_ISIZE: case OXID_VECTOR_USIZE:
    case OXID_VECTOR_F64: return 8;
    case OXID_VECTOR_I128: case OXID_VECTOR_U128: return 16;
    default: return 0;
    }
}
static const oxid_vector_record_schema *vector_record_schema(uint32_t element) {
#ifndef OXID_RECORD_VECTOR_SCHEMAS
    (void)element;
    return NULL;
#else
    if (!(element & OXID_VECTOR_RECORD_BIT)) return NULL;
    uint32_t ordinal = element & ~OXID_VECTOR_RECORD_BIT;
    if (__oxid_record_vector_schema_count > OXID_VECTOR_RECORD_LIMIT ||
        ordinal >= __oxid_record_vector_schema_count) return NULL;
    const oxid_vector_record_schema *schema = &__oxid_record_vector_schemas[ordinal];
    if (schema->element != element || !schema->stride || !schema->owner_bytes ||
        !schema->leaf_count || schema->leaf_count > schema->stride || !schema->leaves) return NULL;
    return schema;
#endif
}
static size_t vector_element_size(uint32_t element) {
    size_t scalar = vector_scalar_size(element);
    if (scalar) return scalar;
    const oxid_vector_record_schema *schema = vector_record_schema(element);
    return schema ? schema->stride : 0;
}
static int vector_record_layout_valid(const oxid_vector_record_schema *schema) {
    size_t dense = 0, owner = 0;
    for (uint32_t i = 0; i < schema->leaf_count; ++i) {
        const oxid_vector_record_leaf *leaf = &schema->leaves[i];
        size_t width = vector_scalar_size(leaf->element);
        if (!width || leaf->dense_offset != dense || leaf->owner_offset < owner ||
            dense > schema->stride || width > schema->stride - dense ||
            leaf->owner_offset > schema->owner_bytes || width > schema->owner_bytes - leaf->owner_offset)
            return 0;
        dense += width;
        owner = (size_t)leaf->owner_offset + width;
    }
    return dense == schema->stride;
}
static int vector_layout(uint32_t element, int32_t capacity, size_t *bytes) {
    size_t width = vector_element_size(element);
    if (!width || capacity < 0 || (size_t)capacity > (SIZE_MAX - 63u) / width)
        return 0;
    size_t size = (48u + (size_t)capacity * width + 15u) & ~(size_t)15u;
    if (size > PTRDIFF_MAX) return 0;
    *bytes = size;
    return 1;
}
static uint8_t *vector_data(oxid_allocation *node) {
    return (uint8_t *)((oxid_vector_header *)node + 1);
}
static int vector_overlaps(const void *a, size_t a_length, const void *b, size_t b_length) {
    if (!a_length || !b_length) return 0;
    uintptr_t left = (uintptr_t)a, right = (uintptr_t)b;
    return left <= right ? right - left < a_length : left - right < b_length;
}
static int vector_scalar_canonical(uint32_t element, const uint8_t *bytes, size_t length) {
    if (element == OXID_VECTOR_BOOL || element == OXID_VECTOR_UNIT) {
        uint8_t maximum = element == OXID_VECTOR_BOOL ? 1 : 0;
        for (size_t i = 0; i < length; ++i) if (bytes[i] > maximum) return 0;
    } else if (element == OXID_VECTOR_CHAR) {
        for (size_t i = 0; i < length; i += 4) {
            uint32_t value = (uint32_t)bytes[i] | (uint32_t)bytes[i + 1] << 8 |
                (uint32_t)bytes[i + 2] << 16 | (uint32_t)bytes[i + 3] << 24;
            if (value > UINT32_C(0x10ffff) ||
                (value >= UINT32_C(0xd800) && value <= UINT32_C(0xdfff))) return 0;
        }
    }
    return 1;
}

static int vector_canonical(uint32_t element, const uint8_t *bytes, size_t length) {
    size_t width = vector_element_size(element);
    if (!width || length % width || (length && !bytes)) return 0;
    if (!(element & OXID_VECTOR_RECORD_BIT)) return vector_scalar_canonical(element, bytes, length);
    if (!length) return 1;
    const oxid_vector_record_schema *schema = vector_record_schema(element);
    if (!schema || !vector_record_layout_valid(schema)) return 0;
    for (size_t offset = 0; offset < length; offset += width) {
        for (uint32_t i = 0; i < schema->leaf_count; ++i) {
            const oxid_vector_record_leaf *leaf = &schema->leaves[i];
            if (!vector_scalar_canonical(leaf->element, bytes + offset + leaf->dense_offset,
                    vector_scalar_size(leaf->element))) return 0;
        }
    }
    return 1;
}

/* A source intersecting a registered allocation must be an initialized dense
 * range of this exact nominal vector, including element-aligned subranges. */
static int vector_record_source(oxid_byte_heap *heap, uint32_t element,
    const uint8_t *source, size_t length) {
    if (!(element & OXID_VECTOR_RECORD_BIT) || !length) return 1;
    size_t width = vector_element_size(element);
    if (!width || length % width) return 0;
    for (oxid_allocation *node = heap->head; node; node = node->next) {
        uint64_t charge = node->capacity == OXID_VECTOR_NODE
            ? ((const oxid_vector_header *)node)->allocation_bytes
#ifdef OXID_OWNED_VECTOR_RUNTIME
            : node->capacity == OXID_OWNED_VECTOR_NODE
                ? oxid_owned_vector_node_charge(node)
#endif
            : OXID_BYTEBUF_HEADER + (node->capacity == UINT32_MAX ? 0 : (uint64_t)node->capacity);
        if (charge > SIZE_MAX) return 0;
        if (!vector_overlaps(source, length, node, (size_t)charge)) continue;
        if (node->capacity != OXID_VECTOR_NODE) return 0;
        const oxid_vector_header *header = (const oxid_vector_header *)node;
        size_t checked_charge;
        if (header->element != element || header->capacity > INT32_MAX ||
            node->initialized > header->capacity || (size_t)node->initialized > SIZE_MAX / width ||
            !vector_layout(element, (int32_t)header->capacity, &checked_charge) || checked_charge != charge) return 0;
        uintptr_t first = (uintptr_t)vector_data(node), selected = (uintptr_t)source;
        size_t initialized = (size_t)node->initialized * width;
        if (selected < first || selected - first > initialized ||
            (selected - first) % width || length > initialized - (selected - first)) return 0;
        return 1;
    }
    return 1;
}

uint64_t oxid_vector_node_charge(const oxid_allocation *node) {
    /* Called only for a registered OXID_VECTOR_NODE; descriptor bytes never
     * select a pointer or an allocation layout for this cleanup operation. */
    return ((const oxid_vector_header *)node)->allocation_bytes;
}
static int32_t locate_vector(oxid_byte_heap *heap, const oxid_vector *value,
    uint32_t element, oxid_allocation **found) {
    if (!heap || !value || !found || !vector_element_size(element)) return OXID_BUFFER_DESCRIPTOR;
    *found = NULL;
    if (!value->identity)
        return !value->length && !value->capacity ? 0 : OXID_BUFFER_DESCRIPTOR;
    if (value->length < 0 || value->capacity <= 0 || value->length > value->capacity)
        return OXID_BUFFER_DESCRIPTOR;
    for (oxid_allocation *node = heap->head; node; node = node->next) {
        if (node->identity == value->identity) {
            if (node->capacity != OXID_VECTOR_NODE || (uintptr_t)node % 16u)
                return OXID_BUFFER_DESCRIPTOR;
            const oxid_vector_header *header = (const oxid_vector_header *)node;
            size_t charge;
            if (!vector_layout(element, value->capacity, &charge) ||
                header->element != element || header->capacity != (uint32_t)value->capacity ||
                node->initialized != (uint32_t)value->length || header->allocation_bytes != charge)
                return OXID_BUFFER_DESCRIPTOR;
            *found = node;
            return 0;
        }
    }
    return OXID_BUFFER_DESCRIPTOR;
}
int32_t oxid_vector_new(uint32_t element, oxid_vector *result) {
    if (!result || !vector_element_size(element)) return OXID_BUFFER_DESCRIPTOR;
    *result = (oxid_vector){0, 0, 0};
    return 0;
}
int32_t oxid_vector_validate(oxid_byte_heap *heap, const oxid_vector *value, uint32_t element) {
    oxid_allocation *node;
    return locate_vector(heap, value, element, &node);
}
int32_t oxid_vector_len(oxid_byte_heap *heap, const oxid_vector *value,
    uint32_t element, int32_t *result) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (!result || vector_overlaps(value, sizeof(*value), result, sizeof(*result)))
        return OXID_BUFFER_DESCRIPTOR;
    *result = value->length;
    return 0;
}
int32_t oxid_vector_capacity(oxid_byte_heap *heap, const oxid_vector *value,
    uint32_t element, int32_t *result) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (!result || vector_overlaps(value, sizeof(*value), result, sizeof(*result)))
        return OXID_BUFFER_DESCRIPTOR;
    *result = value->capacity;
    return 0;
}
int32_t oxid_vector_range(oxid_byte_heap *heap, const oxid_vector *value,
    uint32_t element, int32_t start, int32_t end, uint8_t **bytes) {
    oxid_allocation *node;
    int32_t status = locate_vector(heap, value, element, &node);
    if (status) return status;
    if (start < 0 || end < start || end > value->length) return OXID_BUFFER_BOUNDS;
    if (!bytes || vector_overlaps(value, sizeof(*value), bytes, sizeof(*bytes)))
        return OXID_BUFFER_DESCRIPTOR;
    *bytes = node ? vector_data(node) + (size_t)start * vector_element_size(element) : NULL;
    return 0;
}

enum vector_mutation { VECTOR_RESERVE, VECTOR_EXTEND, VECTOR_RESIZE };
static int32_t change_vector(oxid_byte_heap *heap, oxid_vector *value, uint32_t element,
    int32_t required, enum vector_mutation mutation, const uint8_t *scratch,
    size_t scratch_bytes, uint64_t *fuel) {
    oxid_allocation *old;
    int32_t status = locate_vector(heap, value, element, &old);
    if (status) return status;
    if (!fuel || (scratch_bytes && !scratch) ||
        vector_overlaps(value, sizeof(*value), fuel, sizeof(*fuel)) ||
        vector_overlaps(heap, sizeof(*heap), fuel, sizeof(*fuel)) ||
        (old && vector_overlaps(old, (size_t)oxid_vector_node_charge(old), fuel, sizeof(*fuel))))
        return OXID_BUFFER_DESCRIPTOR;
    int32_t capacity = oxid_bytebuf_growth(value->capacity, required);
    if (capacity < 0) return capacity;
    size_t charge;
    if (!vector_layout(element, capacity, &charge)) return OXID_CAPACITY_OVERFLOW;
    size_t width = vector_element_size(element);
    int grows = capacity != value->capacity;
    int32_t length = mutation == VECTOR_RESERVE ? value->length : required;
    size_t writes = length > value->length ? (size_t)(length - value->length) * width : 0;
    size_t copy_bytes = grows ? (size_t)value->length * width : 0;
    uint64_t validation = (uint64_t)scratch_bytes;
    if (element & OXID_VECTOR_RECORD_BIT) {
        if (validation > UINT64_MAX / OXID_VECTOR_RECORD_VALIDATION_WORK) return OXID_BUFFER_FUEL;
        validation *= OXID_VECTOR_RECORD_VALIDATION_WORK;
    }
    if (validation > UINT64_MAX - writes || validation + writes > UINT64_MAX - copy_bytes)
        return OXID_BUFFER_FUEL;
    uint64_t work = validation + writes + copy_bytes;
    if (vector_overlaps(scratch, scratch_bytes, value, sizeof(*value)) ||
        vector_overlaps(scratch, scratch_bytes, fuel, sizeof(*fuel)) ||
        (old && vector_overlaps(scratch, scratch_bytes, old, (size_t)oxid_vector_node_charge(old))))
        return OXID_BUFFER_DESCRIPTOR;
    if (*fuel < work) return OXID_BUFFER_FUEL;
    if (!vector_record_source(heap, element, scratch, scratch_bytes) ||
        !vector_canonical(element, scratch, scratch_bytes)) return OXID_BUFFER_DESCRIPTOR;
    if (grows && ((uint64_t)charge > heap->limit || heap->ledger.live_bytes > heap->limit - charge))
        return OXID_MEMORY_LIMIT;
    if (grows && heap->next_identity == UINT64_MAX) return OXID_BUFFER_IDENTITY;
    *fuel -= work;
    if (!grows && length == value->length) return OXID_GROW_OK;
    oxid_allocation *target = old;
    if (grows) {
        if (fail(heap, ALLOCATE)) return OXID_ALLOCATION_FAILED;
        oxid_vector_header *header = OXID_VECTOR_ALIGNED_ALLOC(16, charge);
        if (!header) return OXID_ALLOCATION_FAILED;
        *header = (oxid_vector_header){
            {heap->head, NULL, heap->next_identity++, OXID_VECTOR_NODE, 0},
            (uint32_t)capacity, element, charge
        };
        target = &header->prefix;
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
    /* Every refusal precedes initialized payload writes and publication. */
    if (target) {
        if (copy_bytes) memcpy(vector_data(target), vector_data(old), copy_bytes);
        uint8_t *tail = vector_data(target) + (size_t)value->length * width;
        if (writes) {
            if (mutation == VECTOR_EXTEND) memcpy(tail, scratch, writes);
            else if (mutation == VECTOR_RESIZE)
                for (size_t offset = 0; offset < writes; offset += width) memcpy(tail + offset, scratch, width);
        }
        target->initialized = (uint32_t)length;
        value->identity = target->identity;
    }
    value->length = length;
    value->capacity = capacity;
    if (grows && old) unlink_node(heap, old);
    return OXID_GROW_OK;
}
int32_t oxid_vector_reserve(oxid_byte_heap *heap, oxid_vector *value,
    uint32_t element, int32_t additional, uint64_t *fuel) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (additional < 0) return OXID_BUFFER_BOUNDS;
    if (additional > INT32_MAX - value->length) return OXID_CAPACITY_OVERFLOW;
    return change_vector(heap, value, element, value->length + additional, VECTOR_RESERVE, NULL, 0, fuel);
}
int32_t oxid_vector_extend(oxid_byte_heap *heap, oxid_vector *value, uint32_t element,
    const uint8_t *values, int32_t count, uint64_t *fuel) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (count < 0) return OXID_BUFFER_BOUNDS;
    if (count > INT32_MAX - value->length) return OXID_CAPACITY_OVERFLOW;
    size_t width = vector_element_size(element);
    if ((size_t)count > SIZE_MAX / width) return OXID_CAPACITY_OVERFLOW;
    return change_vector(heap, value, element, value->length + count, VECTOR_EXTEND,
        values, (size_t)count * width, fuel);
}
int32_t oxid_vector_push(oxid_byte_heap *heap, oxid_vector *value, uint32_t element,
    const uint8_t *scalar, uint64_t *fuel) {
    return oxid_vector_extend(heap, value, element, scalar, 1, fuel);
}
int32_t oxid_vector_extend_vector(oxid_byte_heap *heap, oxid_vector *value, uint32_t element,
    const oxid_vector *source, int32_t start, int32_t end, uint64_t *fuel) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (!source || !fuel || vector_overlaps(value, sizeof(*value), source, sizeof(*source)) ||
        vector_overlaps(source, sizeof(*source), fuel, sizeof(*fuel))) return OXID_BUFFER_DESCRIPTOR;
    uint8_t *bytes;
    status = oxid_vector_range(heap, source, element, start, end, &bytes);
    if (status) return status;
    if (value->identity && value->identity == source->identity) return OXID_BUFFER_DESCRIPTOR;
    return oxid_vector_extend(heap, value, element, bytes, end - start, fuel);
}
int32_t oxid_vector_resize(oxid_byte_heap *heap, oxid_vector *value, uint32_t element,
    int32_t length, const uint8_t *scalar, uint64_t *fuel) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    return change_vector(heap, value, element, length, VECTOR_RESIZE,
        scalar, vector_element_size(element), fuel);
}
int32_t oxid_vector_truncate(oxid_byte_heap *heap, oxid_vector *value,
    uint32_t element, int32_t length) {
    oxid_allocation *node;
    int32_t status = locate_vector(heap, value, element, &node);
    if (status) return status;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if (length < value->length) {
        if (node) node->initialized = (uint32_t)length;
        value->length = length;
    }
    return 0;
}
int32_t oxid_vector_release(oxid_byte_heap *heap, oxid_vector *value, uint32_t element) {
    oxid_allocation *node;
    int32_t status = locate_vector(heap, value, element, &node);
    if (status) return status;
    if (node) unlink_node(heap, node);
    *value = (oxid_vector){0, 0, 0};
    return 0;
}
int32_t oxid_vector_take(oxid_byte_heap *heap, oxid_vector *value,
    uint32_t element, oxid_vector *result) {
    int32_t status = oxid_vector_validate(heap, value, element);
    if (status) return status;
    if (!result || vector_overlaps(value, sizeof(*value), result, sizeof(*result)))
        return OXID_BUFFER_DESCRIPTOR;
    *result = *value;
    *value = (oxid_vector){0, 0, 0};
    return 0;
}
int32_t oxid_vector_move(oxid_byte_heap *heap, oxid_vector *source,
    uint32_t element, oxid_vector *destination) {
    int32_t status = oxid_vector_validate(heap, source, element);
    if (status) return status;
    status = oxid_vector_validate(heap, destination, element);
    if (status) return status;
    if (destination->identity || vector_overlaps(source, sizeof(*source), destination, sizeof(*destination)))
        return OXID_BUFFER_DESCRIPTOR;
    *destination = *source;
    *source = (oxid_vector){0, 0, 0};
    return 0;
}
