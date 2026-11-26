#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <string.h>
#include "../native/typed_bytebuf.c"
#if !defined(OXID_ARGUMENTS_UTF16)
#define OXID_READ_ENVIRONMENT 1
#endif
#include "../native/experimental_arguments.c"

#if defined(OXID_READ_ENVIRONMENT)
static void environment_contract(void) {
    const uint8_t name[] = "OXID_TEST_C_ENVIRONMENT";
    const size_t length = sizeof(name) - 1;
    const uint64_t cost = sizeof(name) + sizeof("abc") + 3 + 7;
    assert(setenv((const char *)name, "abc", 1) == 0);
    uint8_t bytes[8];
    oxid_binary_result result;
    for (uint64_t budget = 0; budget < cost; ++budget) {
        memset(bytes, 'x', sizeof(bytes));
        result = (oxid_binary_result){99, 98};
        uint64_t fuel = budget;
        assert(__oxid_environment_bytes(name, length, bytes, sizeof(bytes), &fuel, &result, 7)
            == OXID_BUFFER_FUEL);
        assert(fuel == budget && result.tag == 99 && result.value == 98);
        assert(!memcmp(bytes, "xxxxxxxx", sizeof(bytes)));
    }
    uint64_t fuel = cost;
    assert(__oxid_environment_bytes(name, length, bytes, sizeof(bytes), &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && result.value == 3);
    assert(!memcmp(bytes, "abcxxxxx", sizeof(bytes)));
    memset(bytes, 'x', sizeof(bytes));
    fuel = cost - 3;
    assert(__oxid_environment_bytes(name, length, bytes, 2, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 2 && result.value == ERANGE);
    assert(!memcmp(bytes, "xxxxxxxx", sizeof(bytes)));
    fuel = 7;
    assert(__oxid_environment_bytes_hosted(name, length, bytes, sizeof(bytes), &fuel, &result, 7) == 0);
    assert(fuel == 7 && result.tag == 0 && result.value == 3);
    assert(__oxid_environment_bytes(bytes, 3, bytes + 2, 2, &fuel, &result, 7)
        == OXID_BUFFER_DESCRIPTOR);
    assert(fuel == 7 && result.tag == 0 && result.value == 3);
    assert(unsetenv((const char *)name) == 0);
    fuel = sizeof(name) + 7;
    assert(__oxid_environment_bytes(name, length, bytes, sizeof(bytes), &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 1);
}
#endif

int main(void) {
#if defined(OXID_READ_ENVIRONMENT)
    environment_contract();
#endif
#if defined(OXID_ARGUMENTS_UTF16)
    oxid_argument_unit name[] = {'a', 0}, normal[] = {'a', 'b', 'c', 0}, empty[] = {0};
    oxid_argument_unit raw[] = {'y', 'x', 0};
    const uint8_t encoded_raw[] = {'y', 'x'};
#else
    oxid_argument_unit name[] = "application", normal[] = "abc", empty[] = "";
    oxid_argument_unit raw[] = {(char)255, 'x', 0};
    const uint8_t encoded_raw[] = {255, 'x'};
#endif
    oxid_argument_unit *arguments[] = {name, normal, empty, raw, NULL};
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
    assert(!memcmp(bytes, encoded_raw, 2) && !memcmp(bytes + 2, "xxxxxx", 6));
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
    assert(normal[0] == 'a' && normal[1] == 'b' && normal[2] == 'c' && normal[3] == 0);
    assert((uint8_t)raw[0] == encoded_raw[0] && raw[1] == 'x' && raw[2] == 0);
    __oxid_arguments_start(-1, arguments);
    assert(__oxid_arg_count() == 0);
    __oxid_arguments_start(4, NULL);
    assert(__oxid_arg_count() == 0);
    fuel = 7;
    assert(__oxid_arg_bytes(0, bytes, 8, &fuel, &result, 7) == 0 && result.tag == 2);
#if defined(OXID_ARGUMENTS_UTF16)
    oxid_argument_unit wide[] = {0xdc00, 'A', 0x00a2, 0xd83e, 0xdd80, 0xd800, 0};
    oxid_argument_unit *wide_arguments[] = {name, wide, NULL};
    const uint8_t expected[] = "\xef\xbf\xbd" "A\xc2\xa2\xf0\x9f\xa6\x80\xef\xbf\xbd";
    const size_t length = sizeof(expected) - 1;
    uint8_t unicode[16];
    __oxid_arguments_start(2, wide_arguments);
    for (uint64_t budget = 0; budget < 2 * length + 8; ++budget) {
        memset(unicode, 'x', sizeof(unicode));
        result = (oxid_binary_result){99, 98};
        fuel = budget;
        assert(__oxid_arg_bytes(1, unicode, sizeof(unicode), &fuel, &result, 7) == OXID_BUFFER_FUEL);
        assert(fuel == budget && result.tag == 99 && result.value == 98);
        assert(!memcmp(unicode, "xxxxxxxxxxxxxxxx", sizeof(unicode)));
    }
    fuel = length + 8;
    assert(__oxid_arg_bytes(1, unicode, 4, &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 1 && result.value == (int32_t)length);
    assert(!memcmp(unicode, "xxxxxxxxxxxxxxxx", sizeof(unicode)));
    fuel = 2 * length + 8;
    assert(__oxid_arg_bytes(1, unicode, sizeof(unicode), &fuel, &result, 7) == 0);
    assert(fuel == 0 && result.tag == 0 && result.value == (int32_t)length);
    assert(!memcmp(unicode, expected, length) && unicode[length] == 'x');
#endif
    return 0;
}
