/* Private generated-runtime adapter checks, not Oxid native acceptance. */
#include "../native/typed_bytebuf.c"
#include <assert.h>

int main(void) {
    __oxid_bytebuf_start();
    oxid_bytebuf buffer;
    uint64_t remaining = 1000, scalar = 99;
    int32_t grow = -99;
    assert(__oxid_bytebuf_operation(0, NULL, 0, 0, NULL, 0, &remaining, &buffer) == 0);
    assert(buffer.identity == 0 && buffer.length == 0 && buffer.capacity == 0);
    assert(__oxid_bytebuf_operation(6, &buffer, 3, 128, NULL, 0, &remaining, &grow) == 0 && grow == 0);
    assert(__oxid_bytebuf_operation(1, &buffer, 0, 0, NULL, 0, &remaining, &scalar) == 0 && scalar == 3);
    assert(__oxid_bytebuf_operation(2, &buffer, 0, 0, NULL, 0, &remaining, &scalar) == 0 && scalar >= 3);
    uint8_t *view = NULL;
    int32_t length = -99;
    assert(__oxid_bytebuf_view(&buffer, 1, 3, 1, &view, &length) == 0 && length == 2);
    assert(view[0] == 128 && view[1] == 128);
    assert(__oxid_bytebuf_view(&buffer, -1, 3, 1, &view, &length) == OXID_BUFFER_BOUNDS);
    assert(__oxid_bytebuf_operation(4, &buffer, 0, 255, NULL, 0, &remaining, &grow) == 0 && grow == 0);
    const uint8_t tail[] = {0, 1};
    assert(__oxid_bytebuf_operation(5, &buffer, 0, 0, tail, 2, &remaining, &grow) == 0 && grow == 0);
    assert(__oxid_bytebuf_operation(3, &buffer, 30, 0, NULL, 0, &remaining, &grow) == 0 && grow == 0);
    assert(__oxid_bytebuf_view(&buffer, 0, 0, 0, &view, &length) == 0 && length == 6);
    assert(view[3] == 255 && view[4] == 0 && view[5] == 1);
    oxid_binary_result io = {-99, -99};
    assert(__oxid_binary_delivery(8, NULL, 0, &remaining, &io, 7) == 0 && io.tag == 0 && io.value == 0);
    assert(__oxid_binary_delivery(9, NULL, 0, &remaining, &io, 7) == 0 && io.tag == 0 && io.value == 0);
    assert(__oxid_bytebuf_operation(7, &buffer, 0, 0, NULL, 0, &remaining, &scalar) == 0 && scalar == 0);
    __oxid_bytebuf_drop(&buffer);
    assert(__oxid_bytebuf_finish() == 0);
    for (int run = 0; run < 100; ++run) {
        __oxid_bytebuf_start();
        oxid_bytebuf value = {0, 0, 0};
        uint64_t fuel = 100;
        assert(oxid_bytebuf_resize(&execution_heap, &value, 17, 255, &fuel) == OXID_GROW_OK);
        assert(__oxid_bytebuf_valid(&value) == 0);
        oxid_bytebuf result = value;
        value = (oxid_bytebuf){0, 0, 0};
        __oxid_bytebuf_drop(&value);
        assert(execution_heap.ledger.live_allocations == 1);
        __oxid_bytebuf_drop(&result);
        assert(execution_heap.ledger.live_allocations == 0);
        assert(execution_heap.ledger.releases == 1);
        assert(__oxid_bytebuf_finish() == 0);
    }
    __oxid_bytebuf_start();
    oxid_bytebuf staged = {0, 0, 0};
    uint64_t fuel = 100;
    assert(oxid_bytebuf_push(&execution_heap, &staged, 128, &fuel) == OXID_GROW_OK);
    /* End-of-execution checks must expose missed ordinary cleanup before the
     * fatal drain reclaims storage and could otherwise hide the missing drop. */
    assert(__oxid_bytebuf_finish() == OXID_BUFFER_DESCRIPTOR);
    assert(execution_heap.ledger.live_allocations == 0);
    assert(execution_heap.ledger.releases == 1);
    __oxid_bytebuf_cleanup();
    assert(execution_heap.ledger.releases == 1);
    return 0;
}
