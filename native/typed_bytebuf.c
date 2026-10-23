/* Private generated-program adapter. One process entry owns one execution.
 * Included only when a sealed native module declares resource operations. */
#include "experimental_bytebuf.c"

static oxid_byte_heap execution_heap;
void __oxid_bytebuf_start(void) { oxid_heap_init(&execution_heap); }
void __oxid_bytebuf_cleanup(void) { oxid_heap_cleanup(&execution_heap); }
int32_t __oxid_bytebuf_finish(void) {
    int32_t status = execution_heap.ledger.live_allocations == 0 ? 0 : OXID_BUFFER_DESCRIPTOR;
    oxid_heap_cleanup(&execution_heap);
    return status;
}
int32_t __oxid_bytebuf_valid(const oxid_bytebuf *value) {
    return oxid_bytebuf_validate(&execution_heap, value);
}
/* Generated code validates all active leaves before this physical release.
 * Registry unlink/free is infallible and consumes neither allocation nor fuel. */
void __oxid_bytebuf_drop(oxid_bytebuf *value) {
    (void)oxid_bytebuf_release(&execution_heap, value);
}

/* Canonical operation numbers are private to the verified emitter. A single
 * fixed helper performs one primitive only; no source loop lives here. */
int32_t __oxid_bytebuf_operation(int32_t operation, void *buffer, int32_t size,
    int32_t fill, const uint8_t *source, int32_t source_length, uint64_t *fuel,
    void *result) {
    if (!result || !fuel) return OXID_BUFFER_DESCRIPTOR;
    oxid_bytebuf *value = buffer;
    int32_t status;
    switch (operation) {
    case 0: /* new */
        *(oxid_bytebuf *)result = (oxid_bytebuf){0, 0, 0};
        return 0;
    case 1: /* len */
    case 2: /* capacity */
        status = oxid_bytebuf_validate(&execution_heap, value);
        if (status) return status;
        *(uint64_t *)result = (uint32_t)(operation == 1 ? value->length : value->capacity);
        return 0;
    case 3: /* try_reserve */
        status = oxid_bytebuf_reserve(&execution_heap, value, size, fuel);
        break;
    case 4: /* try_push */
        if (fill < 0 || fill > 255) return OXID_BUFFER_DESCRIPTOR;
        status = oxid_bytebuf_push(&execution_heap, value, (uint8_t)fill, fuel);
        break;
    case 5: /* try_extend */
        status = oxid_bytebuf_extend(&execution_heap, value, source, source_length, fuel);
        break;
    case 6: /* try_resize */
        if (fill < 0 || fill > 255) return OXID_BUFFER_DESCRIPTOR;
        status = oxid_bytebuf_resize(&execution_heap, value, size, (uint8_t)fill, fuel);
        break;
    case 7: /* truncate */
        status = oxid_bytebuf_truncate(&execution_heap, value, size);
        if (status) return status;
        *(uint64_t *)result = 0;
        return 0;
    default:
        return OXID_BUFFER_DESCRIPTOR;
    }
    if (status < 0) return status;
    *(int32_t *)result = status;
    return 0;
}

/* One fixed control carrier, admitted separately from the unchanged owner
 * inventory. return_cost is derived by the emitter from the sealed plan. */
typedef struct {
    uint64_t remaining;
    oxid_binary_result value;
} oxid_binary_delivery;
_Static_assert(sizeof(oxid_binary_delivery) == 16, "binary delivery control bytes");
#ifdef OXID_BYTEBUF_TESTING
/* One deterministic component-test refusal, never an execution policy knob. */
static int refuse_binary_delivery;
#endif
int32_t __oxid_binary_delivery(int32_t operation, void *bytes, int32_t length,
    uint64_t *fuel, oxid_binary_result *result, uint64_t return_cost) {
    if (operation != 8 && operation != 9) return OXID_BUFFER_DESCRIPTOR;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if ((length && !bytes) || !fuel || !result) return OXID_BUFFER_DESCRIPTOR;
#ifdef OXID_BYTEBUF_TESTING
    if (refuse_binary_delivery) {
        refuse_binary_delivery = 0;
        return OXID_BUFFER_FUEL;
    }
#endif
    oxid_binary_delivery delivery;
    if (*fuel < (uint64_t)length || *fuel - (uint64_t)length < return_cost)
        return OXID_BUFFER_FUEL;
    delivery.remaining = *fuel - (uint64_t)length - return_cost;
    *fuel = delivery.remaining;
    /* No allocation, admission, generation check or fuel check after this
     * point. The status is committed into the direct caller's result extent. */
    delivery.value = operation == 8 ? read_once(bytes, length) : write_once(bytes, length);
    if (operation == 9 && delivery.value.tag == 2) delivery.value.tag = 1;
    *result = delivery.value;
    return 0;
}

int32_t __oxid_bytebuf_view(const oxid_bytebuf *value, int32_t start, int32_t end,
    int32_t subrange, uint8_t **pointer, int32_t *length) {
    int32_t status = oxid_bytebuf_validate(&execution_heap, value);
    if (status) return status;
    if (!subrange) { start = 0; end = value->length; }
    status = oxid_bytebuf_range(&execution_heap, value, start, end, pointer);
    if (status) return status;
    *length = end - start;
    return 0;
}
