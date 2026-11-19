/* Private typed vector storage. Include with experimental_bytebuf.h.
 * Tags describe a sealed compiler type; a caller-supplied tag is not source
 * authority. All pointer extents and loans must already be admitted by the
 * verified consumer. No scalar crosses this boundary by value. */
#ifndef OXID_EXPERIMENTAL_VECTOR_H
#define OXID_EXPERIMENTAL_VECTOR_H
#include "experimental_bytebuf.h"

#define OXID_VECTOR_NODE (UINT32_MAX - UINT32_C(1))
#define OXID_VECTOR_HEADER UINT64_C(48)
#define OXID_VECTOR_ALIGNMENT 16u

enum oxid_vector_element {
    OXID_VECTOR_BOOL = 1, OXID_VECTOR_UNIT = 2,
    OXID_VECTOR_I8 = 3, OXID_VECTOR_U8 = 4,
    OXID_VECTOR_I16 = 5, OXID_VECTOR_U16 = 6,
    OXID_VECTOR_I32 = 7, OXID_VECTOR_U32 = 8,
    OXID_VECTOR_I64 = 9, OXID_VECTOR_U64 = 10,
    OXID_VECTOR_ISIZE = 11, OXID_VECTOR_USIZE = 12,
    OXID_VECTOR_F32 = 13, OXID_VECTOR_F64 = 14,
    OXID_VECTOR_CHAR = 15, OXID_VECTOR_I128 = 16,
    OXID_VECTOR_U128 = 17
};
typedef struct { uint64_t identity; int32_t length, capacity; } oxid_vector;

/* A closed read-only compiler catalog. Record identities retain their nominal
 * declaration ordinal; no caller supplies a stride, leaf table or callback. */
#define OXID_VECTOR_RECORD_BIT UINT32_C(0x80000000)
#define OXID_VECTOR_RECORD_LIMIT UINT32_C(4096)
#define OXID_VECTOR_RECORD_VALIDATION_WORK UINT64_C(131)
typedef struct { uint32_t dense_offset, owner_offset, element; } oxid_vector_record_leaf;
typedef struct {
    uint32_t element, stride, owner_bytes, leaf_count;
    const oxid_vector_record_leaf *leaves;
} oxid_vector_record_schema;
#ifdef OXID_RECORD_VECTOR_SCHEMAS
extern const uint32_t __oxid_record_vector_schema_count;
extern const oxid_vector_record_schema __oxid_record_vector_schemas[];
#endif


int32_t oxid_vector_new(uint32_t element, oxid_vector *result);
int32_t oxid_vector_validate(oxid_byte_heap *, const oxid_vector *, uint32_t element);
int32_t oxid_vector_len(oxid_byte_heap *, const oxid_vector *, uint32_t element, int32_t *);
int32_t oxid_vector_capacity(oxid_byte_heap *, const oxid_vector *, uint32_t element, int32_t *);
int32_t oxid_vector_range(oxid_byte_heap *, const oxid_vector *, uint32_t element,
    int32_t start, int32_t end, uint8_t **bytes);
int32_t oxid_vector_reserve(oxid_byte_heap *, oxid_vector *, uint32_t element,
    int32_t additional, uint64_t *fuel);
int32_t oxid_vector_push(oxid_byte_heap *, oxid_vector *, uint32_t element,
    const uint8_t *initialized_scalar, uint64_t *fuel);
int32_t oxid_vector_extend(oxid_byte_heap *, oxid_vector *, uint32_t element,
    const uint8_t *initialized_values, int32_t count, uint64_t *fuel);
int32_t oxid_vector_extend_vector(oxid_byte_heap *, oxid_vector *, uint32_t element,
    const oxid_vector *source, int32_t start, int32_t end, uint64_t *fuel);
int32_t oxid_vector_resize(oxid_byte_heap *, oxid_vector *, uint32_t element,
    int32_t length, const uint8_t *initialized_scalar, uint64_t *fuel);
int32_t oxid_vector_truncate(oxid_byte_heap *, oxid_vector *, uint32_t element, int32_t length);
int32_t oxid_vector_release(oxid_byte_heap *, oxid_vector *, uint32_t element);
int32_t oxid_vector_take(oxid_byte_heap *, oxid_vector *, uint32_t element, oxid_vector *result);
int32_t oxid_vector_move(oxid_byte_heap *, oxid_vector *source, uint32_t element,
    oxid_vector *empty_destination);

/* Registry-only cleanup hook. Requires a node allocated by this runtime with
 * capacity == OXID_VECTOR_NODE. Never call with a descriptor-derived address.
 * The byte heap's existing unlink_node subtracts this charge and uses free(). */
uint64_t oxid_vector_node_charge(const oxid_allocation *);
#endif
