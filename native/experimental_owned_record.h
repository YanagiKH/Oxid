/* Read-only compiler facts for finite owned-record vector elements. */
#ifndef OXID_EXPERIMENTAL_OWNED_RECORD_H
#define OXID_EXPERIMENTAL_OWNED_RECORD_H
#include "experimental_vector.h"

enum oxid_owned_type_kind {
    OXID_OWNED_SCALAR = 1, OXID_OWNED_RECORD = 2, OXID_OWNED_ENUM = 3,
    OXID_OWNED_BYTEBUF = 4, OXID_OWNED_FILE = 5, OXID_OWNED_VECTOR = 6,
    OXID_OWNED_ARRAY = 7
};
typedef struct {
    uint32_t kind, detail, first, count;
    uint64_t bytes;
} oxid_owned_type;
typedef struct { uint32_t type, offset; } oxid_owned_field;
typedef struct {
    uint32_t record, root_type;
    uint64_t stride, visits, maximum_resource_leaves;
} oxid_owned_record_schema;

/* Enum variants use declaration-order tags and UINT32_MAX for no payload.
 * Record rows retain nominal declaration ordinals. Ineligible rows also use
 * root_type == UINT32_MAX. Neither a source value nor a caller owns a table. */
#ifdef OXID_OWNED_VECTOR_SCHEMAS
extern const uint32_t __oxid_owned_type_count, __oxid_owned_field_count;
extern const uint32_t __oxid_owned_variant_count, __oxid_owned_record_count;
extern const oxid_owned_type __oxid_owned_types[];
extern const oxid_owned_field __oxid_owned_fields[], __oxid_owned_variants[];
extern const oxid_owned_record_schema __oxid_owned_records[];
#endif
#endif
