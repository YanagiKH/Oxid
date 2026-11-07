/* Independent decimal-output and broken-pipe checks for the native adapter. */
#include <assert.h>
#include <stdint.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>
#include "../native/typed_preview.c"

static void decimal_values(void) {
    int channel[2];
    assert(pipe(channel) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(channel[0]);
        assert(dup2(channel[1], STDOUT_FILENO) == STDOUT_FILENO);
        close(channel[1]);
        const uint64_t values[] = {
            UINT64_C(0), UINT64_C(1), UINT64_C(9), UINT64_C(10), UINT64_C(99), UINT64_C(100), UINT64_C(255), UINT64_C(65535), UINT64_C(2147483648), UINT64_C(4294967295), UINT64_C(4294967296), UINT64_C(9007199254740993), UINT64_C(9223372036854775807), UINT64_C(9223372036854775808), UINT64_C(18446744073709551614), UINT64_C(18446744073709551615)
        };
        for (size_t index = 0; index < sizeof values / sizeof values[0]; ++index)
            if (__oxid_print_u64(values[index]) != 0) _exit(2);
        _exit(0);
    }
    close(channel[1]);
    char buffer[512];
    size_t count = 0;
    for (;;) {
        assert(count < sizeof buffer);
        ssize_t done = read(channel[0], buffer + count, sizeof buffer - count);
        assert(done >= 0);
        if (done == 0) break;
        count += (size_t)done;
    }
    close(channel[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    const char expected[] =
        "0\n"
        "1\n"
        "9\n"
        "10\n"
        "99\n"
        "100\n"
        "255\n"
        "65535\n"
        "2147483648\n"
        "4294967295\n"
        "4294967296\n"
        "9007199254740993\n"
        "9223372036854775807\n"
        "9223372036854775808\n"
        "18446744073709551614\n"
        "18446744073709551615\n"
        ;
    assert(count == sizeof expected - 1);
    assert(memcmp(buffer, expected, count) == 0);
}

static void broken_pipe(void) {
    int channel[2];
    assert(pipe(channel) == 0);
    close(channel[0]);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        assert(dup2(channel[1], STDOUT_FILENO) == STDOUT_FILENO);
        close(channel[1]);
        _exit(__oxid_print_u64(UINT64_MAX) == 74 ? 0 : 3);
    }
    close(channel[1]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
}

int main(void) {
    decimal_values();
    broken_pipe();
    return 0;
}
