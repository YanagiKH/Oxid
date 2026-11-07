/* Private owned-record vector layout. A descriptor grants no type or loan
 * authority; the sealed consumer supplies independently checked record facts. */
#ifndef OXID_EXPERIMENTAL_OWNED_VECTOR_H
#define OXID_EXPERIMENTAL_OWNED_VECTOR_H
#include "experimental_bytebuf.h"
#include "experimental_owned_record.h"

#define OXID_OWNED_VECTOR_NODE (UINT32_MAX - UINT32_C(2))
#define OXID_OWNED_VECTOR_HEADER UINT64_C(64)
#define OXID_OWNED_VECTOR_ALIGNMENT 16u

typedef struct { uint64_t identity; int32_t length, capacity; } oxid_owned_vector;

/* Only a live registry node established by this runtime may reach this raw
 * cleanup hook. It never traverses payload or non-owning identity scratch. */
uint64_t oxid_owned_vector_node_charge(const oxid_allocation *);

/* These private adapters require the sealed consumer's loan and owner proof.
 * Schemas are selected by nominal ordinal; callers never supply type tables.
 * Push consumes initialized source bytes only after a complete preflight.
 * Pop writes unowned result storage. Reserve failure preserves every owner. */
int32_t oxid_owned_vector_new(uint32_t record, oxid_owned_vector *);
int32_t oxid_owned_vector_validate(oxid_byte_heap *, const oxid_owned_vector *,
    uint32_t record, uint64_t *fuel);
/* Normal owner exit uses the same finite descriptor/registry lookup without
 * requiring language fuel. It neither moves nor releases a resource. */
int32_t oxid_owned_vector_validate_maintenance(oxid_byte_heap *,
    const oxid_owned_vector *, uint32_t record);
int32_t oxid_owned_vector_len(oxid_byte_heap *, const oxid_owned_vector *,
    uint32_t record, uint64_t *fuel, int32_t *);
int32_t oxid_owned_vector_capacity(oxid_byte_heap *, const oxid_owned_vector *,
    uint32_t record, uint64_t *fuel, int32_t *);
int32_t oxid_owned_vector_reserve(oxid_byte_heap *, oxid_owned_vector *,
    uint32_t record, int32_t additional, uint64_t *fuel);
int32_t oxid_owned_vector_push_reserved(oxid_byte_heap *, oxid_owned_vector *,
    uint32_t record, uint8_t *source, size_t bytes, uint64_t *fuel);
int32_t oxid_owned_vector_pop_last(oxid_byte_heap *, oxid_owned_vector *,
    uint32_t record, uint8_t *destination, size_t bytes, uint64_t *fuel);
int32_t oxid_owned_vector_truncate(oxid_byte_heap *, oxid_owned_vector *,
    uint32_t record, int32_t length, uint64_t *fuel);
int32_t oxid_owned_vector_take(oxid_byte_heap *, oxid_owned_vector *,
    uint32_t record, uint64_t *fuel, oxid_owned_vector *result);
int32_t oxid_owned_vector_release(oxid_byte_heap *, oxid_owned_vector *, uint32_t record);
#endif
