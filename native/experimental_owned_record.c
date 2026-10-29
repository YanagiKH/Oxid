/* Included in the byte heap translation unit, after the old vector core.
 * The verified compiler supplies immutable finite type facts. Every value walk
 * additionally checks its active shape and exact initialized byte extent. */
#include "experimental_owned_record.h"

#define OXID_OWNED_MAX_DEPTH 64u
typedef struct { uint32_t type, next; size_t offset; } owned_record_frame;
typedef struct {
    const uint8_t *bytes;
    size_t length, depth;
    owned_record_frame frames[OXID_OWNED_MAX_DEPTH + 1u];
} owned_record_cells;
typedef struct { size_t offset, bytes; uint32_t kind, detail; } owned_record_cell;

static const oxid_owned_type *owned_record_type(uint32_t type) {
#ifndef OXID_OWNED_VECTOR_SCHEMAS
    (void)type;
    return NULL;
#else
    return type < __oxid_owned_type_count ? &__oxid_owned_types[type] : NULL;
#endif
}
static const oxid_owned_record_schema *owned_record_schema(uint32_t record) {
#ifndef OXID_OWNED_VECTOR_SCHEMAS
    (void)record;
    return NULL;
#else
    if (record >= __oxid_owned_record_count) return NULL;
    const oxid_owned_record_schema *schema = &__oxid_owned_records[record];
    const oxid_owned_type *root = owned_record_type(schema->root_type);
    if (schema->record != record || !schema->stride || schema->stride > UINT32_MAX ||
        !schema->visits || schema->maximum_resource_leaves > schema->stride / 16u ||
        !root || root->kind != OXID_OWNED_RECORD || root->bytes != schema->stride)
        return NULL;
    return schema;
#endif
}
static const oxid_owned_field *owned_record_member(const oxid_owned_type *type,
    uint32_t index) {
#ifndef OXID_OWNED_VECTOR_SCHEMAS
    (void)type;
    (void)index;
    return NULL;
#else
    if (index >= type->count) return NULL;
    uint32_t available = type->kind == OXID_OWNED_ENUM
        ? __oxid_owned_variant_count : __oxid_owned_field_count;
    if (type->first > available || type->count > available - type->first) return NULL;
    return type->kind == OXID_OWNED_ENUM
        ? &__oxid_owned_variants[type->first + index]
        : &__oxid_owned_fields[type->first + index];
#endif
}
static uint32_t owned_record_u32(const uint8_t *bytes) {
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
        (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}
static int owned_record_extent(const owned_record_cells *cells, size_t offset,
    size_t bytes) {
    return offset <= cells->length && bytes <= cells->length - offset;
}
static int owned_record_push(owned_record_cells *cells, uint32_t type, size_t offset) {
    const oxid_owned_type *value = owned_record_type(type);
    if (!value || value->bytes > SIZE_MAX ||
        !owned_record_extent(cells, offset, (size_t)value->bytes) ||
        cells->depth == OXID_OWNED_MAX_DEPTH + 1u) return 0;
    cells->frames[cells->depth++] = (owned_record_frame){type, 0, offset};
    return 1;
}
static int owned_record_begin(owned_record_cells *cells,
    const oxid_owned_record_schema *schema, const uint8_t *bytes, size_t length) {
    if (!schema || !bytes || schema->stride != length) return 0;
    cells->bytes = bytes;
    cells->length = length;
    cells->depth = 0;
    return owned_record_push(cells, schema->root_type, 0);
}
/* 1 is one active cell, 0 is end, -1 is a refused value or table row. */
static int owned_record_next(owned_record_cells *cells, owned_record_cell *cell) {
    while (cells->depth) {
        owned_record_frame *frame = &cells->frames[cells->depth - 1u];
        const oxid_owned_type *type = owned_record_type(frame->type);
        if (!type) return -1;
        size_t offset = frame->offset;
        if (type->kind == OXID_OWNED_SCALAR) {
            size_t width = vector_scalar_size(type->detail);
            if (!width || type->bytes != width || !owned_record_extent(cells, offset, width))
                return -1;
            --cells->depth;
            *cell = (owned_record_cell){offset, width, OXID_OWNED_SCALAR, type->detail};
            return 1;
        }
        if (type->kind == OXID_OWNED_BYTEBUF || type->kind == OXID_OWNED_FILE ||
            type->kind == OXID_OWNED_VECTOR) {
            if (type->bytes != 16 || !owned_record_extent(cells, offset, 16)) return -1;
            --cells->depth;
            *cell = (owned_record_cell){offset, 16, type->kind, type->detail};
            return 1;
        }
        if (type->kind == OXID_OWNED_ARRAY) {
            size_t width = vector_scalar_size(type->detail);
            uint32_t count = type->count ? type->count : 1u;
            if (!width || count > SIZE_MAX / width || type->bytes != (size_t)count * width)
                return -1;
            if (frame->next == count) { --cells->depth; continue; }
            size_t relative = (size_t)frame->next++ * width;
            if (offset > SIZE_MAX - relative || !owned_record_extent(cells, offset + relative, width))
                return -1;
            *cell = (owned_record_cell){offset + relative, width, OXID_OWNED_SCALAR, type->detail};
            return 1;
        }
        if (type->kind == OXID_OWNED_RECORD) {
            if (!type->count) {
                if (type->bytes != 1 || !owned_record_extent(cells, offset, 1)) return -1;
                --cells->depth;
                *cell = (owned_record_cell){offset, 1, OXID_OWNED_SCALAR, OXID_VECTOR_UNIT};
                return 1;
            }
            if (frame->next == type->count) { --cells->depth; continue; }
            const oxid_owned_field *field = owned_record_member(type, frame->next++);
            if (!field || field->offset > type->bytes || offset > SIZE_MAX - field->offset)
                return -1;
            const oxid_owned_type *child = owned_record_type(field->type);
            if (!child || child->bytes > type->bytes - field->offset ||
                !owned_record_push(cells, field->type, offset + field->offset)) return -1;
            continue;
        }
        if (type->kind == OXID_OWNED_ENUM) {
            if (frame->next) { --cells->depth; continue; }
            if (type->bytes < 4 || !owned_record_extent(cells, offset, 4)) return -1;
            const oxid_owned_field *variant = owned_record_member(type,
                owned_record_u32(cells->bytes + offset));
            if (!variant) return -1;
            frame->next = 1;
            if (variant->type != UINT32_MAX) {
                const oxid_owned_type *child = owned_record_type(variant->type);
                if (!child || variant->offset < 4 || variant->offset > type->bytes ||
                    child->bytes > type->bytes - variant->offset ||
                    offset > SIZE_MAX - variant->offset ||
                    !owned_record_push(cells, variant->type, offset + variant->offset)) return -1;
            }
            *cell = (owned_record_cell){offset, 4, OXID_OWNED_SCALAR, OXID_VECTOR_I32};
            return 1;
        }
        return -1;
    }
    return 0;
}
static int owned_record_resource(oxid_byte_heap *heap, const owned_record_cell *cell,
    const uint8_t *source, uint64_t *identity) {
    memcpy(identity, source, 8);
    if (cell->kind == OXID_OWNED_FILE) {
        uint64_t reserved;
        memcpy(&reserved, source + 8, 8);
        if (reserved) return 0;
        if (!*identity) return 1;
        for (oxid_allocation *node = heap->head; node; node = node->next)
            if (node->identity == *identity)
                return node->capacity == UINT32_MAX && node->initialized <= INT32_MAX;
        return 0;
    }
    if (cell->kind == OXID_OWNED_BYTEBUF) {
        oxid_bytebuf descriptor;
        memcpy(&descriptor, source, 16);
        return oxid_bytebuf_validate(heap, &descriptor) == 0;
    }
    if (cell->kind == OXID_OWNED_VECTOR) {
        oxid_vector descriptor;
        memcpy(&descriptor, source, 16);
        return oxid_vector_validate(heap, &descriptor, cell->detail) == 0;
    }
    return 0;
}
static int owned_record_check(oxid_byte_heap *heap,
    const oxid_owned_record_schema *schema, const uint8_t *bytes, size_t length,
    uint64_t *identities, size_t capacity, size_t *used) {
    owned_record_cells cells;
    owned_record_cell cell;
    if (!used || *used > capacity || (capacity && !identities) ||
        !owned_record_begin(&cells, schema, bytes, length)) return 0;
    int next;
    while ((next = owned_record_next(&cells, &cell)) == 1) {
        const uint8_t *source = bytes + cell.offset;
        if (cell.kind == OXID_OWNED_SCALAR) {
            if (!vector_scalar_canonical(cell.detail, source, cell.bytes)) return 0;
        } else {
            uint64_t identity;
            if (!owned_record_resource(heap, &cell, source, &identity)) return 0;
            if (identity) {
                if (*used == capacity) return 0;
                identities[(*used)++] = identity;
            }
        }
    }
    return next == 0;
}
static int owned_record_copy(const oxid_owned_record_schema *schema,
    const uint8_t *source, uint8_t *destination, size_t length) {
    owned_record_cells cells;
    owned_record_cell cell;
    if (!destination || !owned_record_begin(&cells, schema, source, length)) return 0;
    int next;
    while ((next = owned_record_next(&cells, &cell)) == 1) { }
    if (next != 0) return 0;
    memset(destination, 0, length);
    if (!owned_record_begin(&cells, schema, source, length)) return 0;
    while ((next = owned_record_next(&cells, &cell)) == 1)
        memcpy(destination + cell.offset, source + cell.offset, cell.bytes);
    return next == 0;
}
/* Called only after complete active-shape, kind and uniqueness preflight.
 * Child registrations are independent of the containing vector allocation. */
static void owned_record_release(oxid_byte_heap *heap,
    const oxid_owned_record_schema *schema, const uint8_t *bytes, size_t length) {
    owned_record_cells cells;
    owned_record_cell cell;
    if (!owned_record_begin(&cells, schema, bytes, length)) abort();
    int next;
    while ((next = owned_record_next(&cells, &cell)) == 1) {
        if (cell.kind == OXID_OWNED_SCALAR) continue;
        uint64_t identity;
        memcpy(&identity, bytes + cell.offset, 8);
        if (!identity) continue;
        oxid_allocation *node = heap->head;
        while (node && node->identity != identity) node = node->next;
        if (!node) abort();
        unlink_node(heap, node);
    }
    if (next != 0) abort();
}
/* In-place heapsort has bounded O(n log n) work and no allocator dependency. */
static void owned_record_sift(uint64_t *values, size_t root, size_t count) {
    while (count > 1 && root <= (count - 2u) / 2u) {
        size_t child = root * 2u + 1u;
        if (child + 1u < count && values[child] < values[child + 1u]) ++child;
        if (values[root] >= values[child]) return;
        uint64_t old = values[root]; values[root] = values[child]; values[child] = old;
        root = child;
    }
}
static int owned_record_unique(uint64_t *identities, size_t count) {
    if (count && !identities) return 0;
    for (size_t i = count / 2u; i; --i) owned_record_sift(identities, i - 1u, count);
    for (size_t end = count; end > 1u; --end) {
        uint64_t old = identities[0]; identities[0] = identities[end - 1u]; identities[end - 1u] = old;
        owned_record_sift(identities, 0, end - 1u);
    }
    for (size_t i = 1; i < count; ++i) if (identities[i - 1u] == identities[i]) return 0;
    return 1;
}
