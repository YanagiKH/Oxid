/* Component checks for canonical direct delivery, not native Oxid execution. */
#include <assert.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>
static ssize_t injected_read(int fd, void *bytes, size_t length);
static ssize_t injected_write(int fd, const void *bytes, size_t length);
#define read injected_read
#define write injected_write
#include "../native/typed_bytebuf.c"
#undef read
#undef write

static int calls, error_code;
static ssize_t progress;
static uint8_t output[8];
static size_t output_length;
static const uint8_t input[] = {0, 128, 255};
static ssize_t injected_read(int fd, void *bytes, size_t length) {
    assert(fd == STDIN_FILENO);
    ++calls;
    if (progress < 0) { errno = error_code; return -1; }
    assert((size_t)progress <= length && (size_t)progress <= sizeof input);
    memcpy(bytes, input, (size_t)progress);
    return progress;
}
static ssize_t injected_write(int fd, const void *bytes, size_t length) {
    assert(fd == STDOUT_FILENO);
    ++calls;
    if (progress < 0) { errno = error_code; return -1; }
    assert((size_t)progress <= length && (size_t)progress <= sizeof output);
    output_length = (size_t)progress;
    memcpy(output, bytes, output_length);
    return progress;
}

static void real_delivery_thresholds(int operation) {
    const uint64_t return_cost = 17;
    const uint64_t refused[] = {0, 3, 4, 20};
    /* Four bytes plus delivery cost: sufficient byte fuel alone must not
     * authorize an attempt whose result cannot be delivered. */
    for (size_t i = 0; i < sizeof refused / sizeof *refused; ++i) {
        uint8_t bytes[4] = {99, 99, 99, 99};
        const uint8_t before[4] = {99, 99, 99, 99};
        uint64_t fuel = refused[i];
        oxid_binary_result result = {77, 88};
        calls = 0; progress = 3; output_length = sizeof output;
        memset(output, 165, sizeof output);
        assert(__oxid_binary_delivery(operation, bytes, 4, &fuel, &result,
                                      return_cost) == OXID_BUFFER_FUEL);
        assert(calls == 0 && fuel == refused[i]);
        assert(result.tag == 77 && result.value == 88);
        assert(memcmp(bytes, before, sizeof bytes) == 0);
        assert(output_length == sizeof output);
        for (size_t j = 0; j < sizeof output; ++j) assert(output[j] == 165);
    }
    {
        uint8_t bytes[4] = {99, 99, 99, 99};
        const uint8_t before[4] = {99, 99, 99, 99};
        uint64_t fuel = 4 + return_cost;
        oxid_binary_result result = {77, 88};
        calls = 0; progress = 3; output_length = 0;
        assert(__oxid_binary_delivery(operation, bytes, 4, &fuel, &result,
                                      return_cost) == 0);
        assert(calls == 1 && fuel == 0 && result.tag == 0 && result.value == 3);
        if (operation == 8) {
            assert(memcmp(bytes, input, sizeof input) == 0 && bytes[3] == 99);
            assert(output_length == 0);
        } else {
            assert(memcmp(bytes, before, sizeof bytes) == 0);
            assert(output_length == 3 && memcmp(output, before, 3) == 0);
        }
    }
    {
        uint64_t fuel = return_cost - 1;
        oxid_binary_result result = {77, 88};
        calls = 0;
        assert(__oxid_binary_delivery(operation, NULL, 0, &fuel, &result,
                                      return_cost) == OXID_BUFFER_FUEL);
        assert(calls == 0 && fuel == return_cost - 1);
        assert(result.tag == 77 && result.value == 88);
        fuel = return_cost;
        assert(__oxid_binary_delivery(operation, NULL, 0, &fuel, &result,
                                      return_cost) == 0);
        assert(calls == 0 && fuel == 0 && result.tag == 0 && result.value == 0);
    }
    {
        uint8_t bytes[4] = {99, 99, 99, 99};
        uint64_t fuel = UINT64_MAX;
        oxid_binary_result result = {77, 88};
        calls = 0; progress = 3; output_length = sizeof output;
        memset(output, 165, sizeof output);
        /* Adding the byte charge to this return cost would wrap. */
        assert(__oxid_binary_delivery(operation, bytes, 4, &fuel, &result,
                                      UINT64_MAX) == OXID_BUFFER_FUEL);
        assert(calls == 0 && fuel == UINT64_MAX);
        assert(result.tag == 77 && result.value == 88);
        for (size_t j = 0; j < sizeof bytes; ++j) assert(bytes[j] == 99);
        assert(output_length == sizeof output);
        for (size_t j = 0; j < sizeof output; ++j) assert(output[j] == 165);
        assert(__oxid_binary_delivery(operation, NULL, 0, &fuel, &result,
                                      UINT64_MAX) == 0);
        assert(calls == 0 && fuel == 0 && result.tag == 0 && result.value == 0);
    }
}

int main(void) {
    const uint64_t delivery_cost = 17; /* arbitrary derived-cost parameter */
    for (int operation = 8; operation <= 9; ++operation) {
        uint8_t bytes[4] = {99, 99, 99, 99};
        uint64_t fuel = 1000;
        oxid_binary_result result = {77, 88};
        calls = 0;
        refuse_binary_delivery = 1;
        assert(__oxid_binary_delivery(operation, bytes, 4, &fuel, &result, delivery_cost) == OXID_BUFFER_FUEL);
        assert(calls == 0 && fuel == 1000 && result.tag == 77 && result.value == 88);
        for (size_t i = 0; i < sizeof bytes; ++i) assert(bytes[i] == 99);
        assert(__oxid_binary_delivery(operation, bytes, -1, &fuel, &result, delivery_cost) == OXID_BUFFER_BOUNDS);
        assert(__oxid_binary_delivery(operation, bytes, 4, &fuel, NULL, delivery_cost) == OXID_BUFFER_DESCRIPTOR);
        assert(__oxid_binary_delivery(operation, NULL, 4, &fuel, &result, delivery_cost) == OXID_BUFFER_DESCRIPTOR);
        assert(calls == 0 && fuel == 1000 && result.tag == 77 && result.value == 88);
        /* The old local-result adapter cannot bypass delivery admission. */
        assert(__oxid_bytebuf_operation(operation, bytes, 4, 0, NULL, 0, &fuel, &result) == OXID_BUFFER_DESCRIPTOR);
        assert(calls == 0 && fuel == 1000);
        assert(__oxid_binary_delivery(operation, NULL, 0, &fuel, &result, delivery_cost) == 0);
        assert(calls == 0 && result.tag == 0 && result.value == 0);
        assert(fuel == 1000 - delivery_cost);
    }
    for (int operation = 8; operation <= 9; ++operation)
        real_delivery_thresholds(operation);
    const ssize_t outcomes[] = {3, 0, -1, -1};
    for (size_t i = 0; i < sizeof outcomes / sizeof *outcomes; ++i) {
        uint8_t bytes[4] = {99, 99, 99, 99};
        uint64_t fuel = 1000;
        oxid_binary_result result = {77, 88};
        progress = outcomes[i]; error_code = i == 2 ? EINTR : EBADF; calls = 0;
        assert(__oxid_binary_delivery(8, bytes, 4, &fuel, &result, delivery_cost) == 0);
        assert(calls == 1 && fuel == 1000 - 4 - delivery_cost);
        assert(result.tag == (progress < 0 ? 2 : progress == 0 ? 1 : 0));
        assert(result.value == (progress < 0 ? error_code : progress));
        assert(bytes[3] == 99);
        if (progress > 0) assert(memcmp(bytes, input, sizeof input) == 0);
        else for (size_t j = 0; j < sizeof bytes; ++j) assert(bytes[j] == 99);
    }
    const ssize_t writes[] = {2, 0, -1, -1};
    for (size_t i = 0; i < sizeof writes / sizeof *writes; ++i) {
        uint64_t fuel = 1000;
        oxid_binary_result result = {77, 88};
        progress = writes[i]; error_code = i == 2 ? EINTR : EPIPE;
        calls = 0; output_length = 0;
        uint8_t bytes[3]; memcpy(bytes, input, sizeof input);
        assert(__oxid_binary_delivery(9, bytes, 3, &fuel, &result, delivery_cost) == 0);
        assert(calls == 1 && fuel == 1000 - 3 - delivery_cost);
        assert(result.tag == (progress < 0 ? 1 : 0));
        assert(result.value == (progress < 0 ? error_code : progress));
        assert(output_length == (progress < 0 ? 0 : (size_t)progress));
        assert(memcmp(output, input, output_length) == 0);
        assert(memcmp(bytes, input, sizeof input) == 0);
    }
    return 0;
}
