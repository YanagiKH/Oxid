/* Private generated-program vector adapter. Included after the real vector
 * core and execution_heap. Scalar arguments are initialized typed byte extents;
 * no i128 or floating-point scalar crosses this C ABI by value. */
typedef struct {
    uint64_t remaining;
    oxid_vector transferred;
    uint64_t scalar;
    int32_t status;
    size_t result_bytes;
} oxid_vector_delivery;
_Static_assert(sizeof(oxid_vector_delivery) == 48, "vector delivery control");

static int vector_heap_overlap(const void *pointer, size_t length) {
    if (vector_overlaps(pointer, length, &execution_heap, sizeof execution_heap)) return 1;
    for (oxid_allocation *node = execution_heap.head; node; node = node->next) {
        uint64_t charge = node->capacity == OXID_VECTOR_NODE ? oxid_vector_node_charge(node)
            : OXID_BYTEBUF_HEADER + (node->capacity == UINT32_MAX ? 0 : (uint64_t)node->capacity);
        if (charge > SIZE_MAX || vector_overlaps(pointer, length, node, (size_t)charge)) return 1;
    }
    return 0;
}
static int vector_output(const void *output, size_t bytes, const oxid_vector *value) {
    return output && !vector_heap_overlap(output, bytes)
        && (!value || !vector_overlaps(output, bytes, value, sizeof *value));
}
static size_t vector_result_bytes(int32_t operation) {
    if (operation < 0 || operation > 8) return 0;
    return operation == 0 || operation == 8 ? 16
        : operation == 1 || operation == 2 || operation == 7 ? 8 : 4;
}

/* Operation numbers are closed emitter facts: new/len/capacity/reserve/push/
 * extend/resize/truncate/take = 0..8. All result locations are preadmitted by
 * the compiler; these overlap checks additionally refuse malformed private
 * C caller roles before a return prepayment can modify initialized storage. */
int32_t __oxid_vector_operation(int32_t operation, uint32_t element,
    oxid_vector *value, int32_t count, const uint8_t *scalar,
    const uint8_t *source, int32_t source_count, uint64_t *fuel,
    void *result, uint64_t return_cost) {
    oxid_vector_delivery delivery;
    size_t width = vector_element_size(element), scalar_bytes = 0, source_bytes = 0;
    if (operation < 0 || operation > 8 || !width || !fuel
        || (uintptr_t)fuel % _Alignof(uint64_t)) return OXID_BUFFER_DESCRIPTOR;
    delivery.result_bytes = vector_result_bytes(operation);
    if (operation != 0 && (!value || (uintptr_t)value % _Alignof(oxid_vector))) return OXID_BUFFER_DESCRIPTOR;
    if (!vector_output(result, delivery.result_bytes, operation == 0 ? NULL : value)
        || vector_heap_overlap(fuel, sizeof *fuel)
        || vector_overlaps(result, delivery.result_bytes, fuel, sizeof *fuel)
        || (operation != 0 && vector_overlaps(value, sizeof *value, fuel, sizeof *fuel))) return OXID_BUFFER_DESCRIPTOR;
    if (operation == 4 || operation == 6) scalar_bytes = width;
    if (operation == 5 && source_count >= 0) {
        if ((size_t)source_count > SIZE_MAX / width) return OXID_BUFFER_BOUNDS;
        source_bytes = (size_t)source_count * width;
    }
    if ((scalar_bytes && !scalar) || (source_bytes && !source)
        || vector_overlaps(result, delivery.result_bytes, scalar, scalar_bytes)
        || vector_overlaps(result, delivery.result_bytes, source, source_bytes)
        || vector_overlaps(fuel, sizeof *fuel, scalar, scalar_bytes)
        || vector_overlaps(fuel, sizeof *fuel, source, source_bytes)) return OXID_BUFFER_DESCRIPTOR;
    if (operation != 0) {
        delivery.status = oxid_vector_validate(&execution_heap, value, element);
        if (delivery.status) return delivery.status;
    }
    if (*fuel < return_cost) return OXID_BUFFER_FUEL;
    delivery.remaining = *fuel - return_cost;
    *fuel = delivery.remaining;
    switch (operation) {
    case 0:
        delivery.transferred = (oxid_vector){0, 0, 0};
        memcpy(result, &delivery.transferred, 16);
        return 0;
    case 1: case 2:
        delivery.scalar = (uint32_t)(operation == 1 ? value->length : value->capacity);
        memcpy(result, &delivery.scalar, 8);
        return 0;
    case 3: delivery.status = oxid_vector_reserve(&execution_heap, value, element, count, fuel); break;
    case 4: delivery.status = oxid_vector_push(&execution_heap, value, element, scalar, fuel); break;
    case 5: delivery.status = oxid_vector_extend(&execution_heap, value, element, source, source_count, fuel); break;
    case 6: delivery.status = oxid_vector_resize(&execution_heap, value, element, count, scalar, fuel); break;
    case 7:
        delivery.status = oxid_vector_truncate(&execution_heap, value, element, count);
        if (delivery.status) return delivery.status;
        delivery.scalar = 0;
        memcpy(result, &delivery.scalar, 8);
        return 0;
    case 8:
        delivery.status = oxid_vector_take(&execution_heap, value, element, &delivery.transferred);
        if (delivery.status) return delivery.status;
        memcpy(result, &delivery.transferred, 16);
        return 0;
    default: return OXID_BUFFER_DESCRIPTOR;
    }
    if (delivery.status < 0) return delivery.status;
    /* Heap transaction/return admission has finished. Publishing the closed
     * status cannot allocate, fail, charge fuel, or inspect caller metadata. */
    memcpy(result, &delivery.status, 4);
    return 0;
}
int32_t __oxid_vector_operation_hosted(int32_t operation, uint32_t element,
    oxid_vector *value, int32_t count, const uint8_t *scalar,
    const uint8_t *source, int32_t source_count, uint64_t *fuel,
    void *result, uint64_t return_cost) {
    if (!fuel || (uintptr_t)fuel % _Alignof(uint64_t)
        || vector_heap_overlap(fuel, sizeof *fuel)) return OXID_BUFFER_DESCRIPTOR;
    size_t output_bytes=vector_result_bytes(operation), width=vector_element_size(element);
    if (!output_bytes || !width) return OXID_BUFFER_DESCRIPTOR;
    size_t scalar_bytes=operation==4 || operation==6 ? width : 0;
    size_t source_bytes=0;
    if (operation==5 && source_count>=0) {
        if ((size_t)source_count>SIZE_MAX/width) return OXID_BUFFER_BOUNDS;
        source_bytes=(size_t)source_count*width;
    }
    /* The fresh hosted budget must not erase the original caller-role checks:
     * it is still invalid to publish into or read operands from the fuel word. */
    if (vector_overlaps(fuel,sizeof *fuel,result,output_bytes)
        || (operation!=0 && vector_overlaps(fuel,sizeof *fuel,value,sizeof *value))
        || vector_overlaps(fuel,sizeof *fuel,scalar,scalar_bytes)
        || vector_overlaps(fuel,sizeof *fuel,source,source_bytes)) return OXID_BUFFER_DESCRIPTOR;
    uint64_t operation_budget = UINT64_MAX;
    return __oxid_vector_operation(operation, element, value, count, scalar, source,
        source_count, &operation_budget, result, return_cost);
}

/* These output extents are the existing paid native scalar/loan slots or the
 * one separately admitted reusable pointer scratch. Counts remain elements. */
int32_t __oxid_vector_view(const oxid_vector *value, uint32_t element,
    int32_t start, int32_t end, int32_t subrange, uint8_t **pointer, int32_t *length) {
    if (!value || (uintptr_t)value % _Alignof(oxid_vector)
        || !vector_output(pointer, sizeof *pointer, value)
        || !vector_output(length, sizeof *length, value)
        || vector_overlaps(pointer, sizeof *pointer, length, sizeof *length)) return OXID_BUFFER_DESCRIPTOR;
    int32_t status = oxid_vector_validate(&execution_heap, value, element);
    if (status) return status;
    if (!subrange) { start = 0; end = value->length; }
    uint8_t *bytes;
    status = oxid_vector_range(&execution_heap, value, element, start, end, &bytes);
    if (status) return status;
    int32_t count = end - start;
    memcpy(pointer, &bytes, sizeof bytes);
    memcpy(length, &count, sizeof count);
    return 0;
}
int32_t __oxid_vector_index(const oxid_vector *value, uint32_t element,
    int32_t ordinal, uint8_t **pointer) {
    if (!value || (uintptr_t)value % _Alignof(oxid_vector)
        || !vector_output(pointer, sizeof *pointer, value)) return OXID_BUFFER_DESCRIPTOR;
    int32_t status = oxid_vector_validate(&execution_heap, value, element);
    if (status) return status;
    if (ordinal < 0 || ordinal >= value->length) return OXID_BUFFER_BOUNDS;
    uint8_t *bytes;
    status = oxid_vector_range(&execution_heap, value, element, ordinal, ordinal + 1, &bytes);
    if (status) return status;
    memcpy(pointer, &bytes, sizeof bytes);
    return 0;
}
int32_t __oxid_vector_length(const oxid_vector *value, uint32_t element, void *result) {
    if (!value || (uintptr_t)value % _Alignof(oxid_vector)
        || !vector_output(result, 8, value)) return OXID_BUFFER_DESCRIPTOR;
    int32_t status = oxid_vector_validate(&execution_heap, value, element);
    if (status) return status;
    uint64_t length = (uint32_t)value->length;
    memcpy(result, &length, sizeof length);
    return 0;
}
