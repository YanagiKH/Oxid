/* Hosted primitives retain the bounded primitive's validation and delivery.
 * Transport below is deterministic component-test input, not native Oxid I/O. */
#include <assert.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>
static ssize_t scripted_read(int, void *, size_t);
static ssize_t scripted_write(int, const void *, size_t);
#define read scripted_read
#define write scripted_write
#include "../native/typed_bytebuf.c"
#undef read
#undef write

static size_t calls;
static ssize_t progress = 3;
static int error_code = EBADF;
static uint8_t written[4];
static const uint8_t input[3] = {0, 128, 255};
static ssize_t scripted_read(int fd, void *bytes, size_t length) {
    assert(fd == STDIN_FILENO);
    ++calls;
    if (progress < 0) { errno = error_code; return -1; }
    assert((size_t)progress <= length && (size_t)progress <= sizeof input);
    memcpy(bytes, input, (size_t)progress);
    return progress;
}
static ssize_t scripted_write(int fd, const void *bytes, size_t length) {
    assert(fd == STDOUT_FILENO);
    ++calls;
    if (progress < 0) { errno = error_code; return -1; }
    assert((size_t)progress <= length && (size_t)progress <= sizeof written);
    memcpy(written, bytes, (size_t)progress);
    return progress;
}

static void growth(void) {
    uint64_t caller = 0, measured = UINT64_MAX;
    oxid_bytebuf hosted = {0}, bounded = {0};
    int32_t status = 77;
    __oxid_bytebuf_start();
    assert(__oxid_bytebuf_operation_hosted(0, NULL, 0, 0, NULL, 0, &caller, &hosted) == 0);
    assert(__oxid_bytebuf_operation(0, NULL, 0, 0, NULL, 0, &measured, &bounded) == 0);
    for (int32_t i = 0; i < 500001; ++i) {
        int32_t byte = i & 255;
        assert(__oxid_bytebuf_operation_hosted(4, &hosted, 0, byte, NULL, 0, &caller, &status) == 0);
        assert(status == OXID_GROW_OK && caller == 0);
        assert(__oxid_bytebuf_operation(4, &bounded, 0, byte, NULL, 0, &measured, &status) == 0);
        assert(status == OXID_GROW_OK);
    }
    assert(UINT64_MAX - measured > 1000000);
    assert(hosted.length == 500001 && hosted.length == bounded.length);
    uint8_t *a, *b;
    int32_t a_length, b_length;
    assert(__oxid_bytebuf_view(&hosted, 0, 0, 0, &a, &a_length) == 0);
    assert(__oxid_bytebuf_view(&bounded, 0, 0, 0, &b, &b_length) == 0);
    assert(a_length == b_length && memcmp(a, b, (size_t)a_length) == 0);
    for (int32_t i = 0; i < a_length; ++i) assert(a[i] == (uint8_t)i);
    oxid_bytebuf before = hosted;
    status = 77;
    assert(__oxid_bytebuf_operation_hosted(4, &hosted, 0, 256, NULL, 0, &caller, &status) == OXID_BUFFER_DESCRIPTOR);
    assert(status == 77 && caller == 0 && memcmp(&hosted, &before, sizeof before) == 0);
    assert(__oxid_bytebuf_operation_hosted(4, &hosted, 0, 1, NULL, 0, NULL, &status) == OXID_BUFFER_DESCRIPTOR);
    assert(status == 77 && memcmp(&hosted, &before, sizeof before) == 0);
    __oxid_bytebuf_drop(&hosted);
    __oxid_bytebuf_drop(&bounded);
    assert(execution_heap.ledger.live_allocations == 0);
    assert(__oxid_bytebuf_finish() == 0);
}

static void delivery(int operation) {
    uint8_t bytes[4] = {99, 99, 99, 99};
    uint64_t caller = 0;
    oxid_binary_result result = {77, 88};
    calls = 0; progress = 3;
    for (size_t i = 0; i < 50001; ++i) {
        assert(__oxid_binary_delivery_hosted(operation, bytes, 4, &caller, &result, 17) == 0);
        assert(caller == 0 && result.tag == 0 && result.value == 3);
    }
    assert(calls == 50001 && (uint64_t)calls * (4 + 17) > 1000000);
    if (operation == 8) assert(memcmp(bytes, input, sizeof input) == 0 && bytes[3] == 99);
    else assert(memcmp(written, bytes, 3) == 0);
    uint64_t bounded = 1000000;
    calls = 0;
    for (;;) {
        result = (oxid_binary_result){77, 88};
        int32_t status = __oxid_binary_delivery(operation, bytes, 4, &bounded, &result, 17);
        if (status) {
            assert(status == OXID_BUFFER_FUEL && result.tag == 77 && result.value == 88);
            break;
        }
    }
    assert(calls == 1000000 / 21 && bounded == 1000000 % 21);
    const ssize_t outcomes[] = {0, -1, -1};
    for (size_t i = 0; i < sizeof outcomes / sizeof *outcomes; ++i) {
        progress = outcomes[i]; error_code = i == 1 ? EINTR : EBADF; calls = 0;
        assert(__oxid_binary_delivery_hosted(operation, bytes, 4, &caller, &result, 17) == 0);
        assert(caller == 0 && calls == 1);
        if (progress < 0) assert(result.tag == (operation == 8 ? 2 : 1) && result.value == error_code);
        else assert(result.tag == (operation == 8 ? 1 : 0) && result.value == 0);
    }
    calls = 0; result = (oxid_binary_result){77, 88};
    assert(__oxid_binary_delivery_hosted(operation, bytes, 4, &caller, &result, UINT64_MAX) == OXID_BUFFER_FUEL);
    assert(caller == 0 && calls == 0 && result.tag == 77 && result.value == 88);
    assert(__oxid_binary_delivery_hosted(operation, bytes, -1, &caller, &result, 17) == OXID_BUFFER_BOUNDS);
    assert(__oxid_binary_delivery_hosted(operation, NULL, 4, &caller, &result, 17) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_binary_delivery_hosted(operation, bytes, 4, NULL, &result, 17) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_binary_delivery_hosted(operation, bytes, 4, &caller, NULL, 17) == OXID_BUFFER_DESCRIPTOR);
    assert(caller == 0 && calls == 0 && result.tag == 77 && result.value == 88);
    refuse_binary_delivery = 1;
    assert(__oxid_binary_delivery_hosted(operation, bytes, 4, &caller, &result, 17) == OXID_BUFFER_FUEL);
    assert(caller == 0 && calls == 0 && result.tag == 77 && result.value == 88);
    assert(__oxid_binary_delivery_hosted(operation, NULL, 0, &caller, &result, 17) == 0);
    assert(caller == 0 && calls == 0 && result.tag == 0 && result.value == 0);
}

int main(void) {
    growth();
    delivery(8);
    delivery(9);
    return 0;
}
