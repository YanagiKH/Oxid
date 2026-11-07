#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <string.h>
#include "../native/typed_bytebuf.c"
#include "../native/experimental_arguments.c"

int main(void) {
    char name[] = "application", normal[] = "abc", empty[] = "", raw[] = { (char)255, 'x', 0 };
    char *arguments[] = {name, normal, empty, raw, NULL};
    __oxid_arguments_start(4, arguments);
    assert(__oxid_arg_count() == 4);
    uint8_t bytes[8];
    oxid_binary_result result;
    for (uint64_t budget = 0; budget < 14; ++budget) {
        memset(bytes, 'x', sizeof(bytes));
        result = (oxid_binary_result){99, 98};
        uint64_t fuel = budget;
        assert(__oxid_arg_bytes(1, bytes, sizeof(bytes), &fuel, &result, 7) == OXID_BUFFER_FUEL);
        assert(fuel == budget && result.tag == 99 && result.value == 98);
        assert(!memcmp(bytes, "xxxxxxxx", 8));
    }
    uint64_t fuel = 14;
    assert(__oxid_arg_bytes(1, bytes, 8, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && result.value == 3 && !memcmp(bytes, "abcxxxxx", 8));
    memset(bytes, 'x', sizeof(bytes));
    fuel = 11;
    assert(__oxid_arg_bytes(1, bytes, 2, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 1 && result.value == 3 && !memcmp(bytes, "xxxxxxxx", 8));
    fuel = 8;
    assert(__oxid_arg_bytes(2, NULL, 0, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && result.value == 0);
    for (int32_t index = -1; index <= 4; index += 5) {
        fuel = 7;
        assert(__oxid_arg_bytes(index, bytes, 8, &fuel, &result, 7) == 0);
        assert(fuel == 0 && result.tag == 2 && !memcmp(bytes, "xxxxxxxx", 8));
    }
    fuel = 12;
    assert(__oxid_arg_bytes(3, bytes, 8, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && result.value == 2);
    assert(bytes[0] == 255 && bytes[1] == 'x' && !memcmp(bytes + 2, "xxxxxx", 6));
    fuel = 7;
    assert(__oxid_arg_bytes_hosted(1, bytes, 8, &fuel, &result, 7) == 0);
    assert(fuel == 7 && result.tag == 0 && !memcmp(bytes, "abcxxxxx", 8));
    assert(__oxid_arg_bytes(1, bytes, -1, &fuel, &result, 7) == OXID_BUFFER_BOUNDS);
    assert(__oxid_arg_bytes(1, NULL, 1, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_arg_bytes(1, (uint8_t *)&result, 1, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_arg_bytes(1, (uint8_t *)&fuel, 1, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
    assert(__oxid_arg_bytes(1, bytes, 8, &fuel, (oxid_binary_result *)&fuel, 7) == OXID_BUFFER_DESCRIPTOR);
    typedef int32_t (*argument_attempt)(int32_t, uint8_t *, int32_t,
        uint64_t *, oxid_binary_result *, uint64_t);
    argument_attempt attempts[] = {__oxid_arg_bytes, __oxid_arg_bytes_hosted};
    for (size_t index = 0; index < sizeof(attempts) / sizeof(attempts[0]); ++index) {
        argument_attempt attempt = attempts[index];
        fuel = 17;
        memset(bytes, 'x', sizeof(bytes));
        result = (oxid_binary_result){99, 98};
        assert(attempt(1, bytes, 8, &fuel, (oxid_binary_result *)&fuel, 7) == OXID_BUFFER_DESCRIPTOR);
        assert(fuel == 17 && !memcmp(bytes, "xxxxxxxx", 8));
        assert(attempt(1, (uint8_t *)&fuel, 1, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
        assert(fuel == 17 && result.tag == 99 && result.value == 98);
        assert(attempt(1, (uint8_t *)&result, 1, &fuel, &result, 7) == OXID_BUFFER_DESCRIPTOR);
        assert(fuel == 17 && result.tag == 99 && result.value == 98);
    }
    assert(!memcmp(normal, "abc", 4) && !memcmp(raw, "\xffx", 3));
    __oxid_arguments_start(-1, arguments);
    assert(__oxid_arg_count() == 0);
    __oxid_arguments_start(4, NULL);
    assert(__oxid_arg_count() == 0);
    fuel = 7;
    assert(__oxid_arg_bytes(0, bytes, 8, &fuel, &result, 7) == 0 && result.tag == 2);
    return 0;
}
