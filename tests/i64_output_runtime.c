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
        const int64_t values[] = {
            INT64_MIN, INT64_MIN + 1, -INT64_C(9007199254740993), -1,
            0, 1, INT64_C(9007199254740993), INT64_MAX - 1, INT64_MAX,
        };
        for (size_t index = 0; index < sizeof values / sizeof values[0]; ++index)
            if (__oxid_print_i64(values[index]) != 0) _exit(2);
        _exit(0);
    }
    close(channel[1]);
    char buffer[256];
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
        "-9223372036854775808\n-9223372036854775807\n-9007199254740993\n-1\n"
        "0\n1\n9007199254740993\n9223372036854775806\n9223372036854775807\n";
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
        _exit(__oxid_print_i64(INT64_MIN) == 74 ? 0 : 3);
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
