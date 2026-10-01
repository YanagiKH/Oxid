/* Experimental Linux scalar output adapter. No Oxid/LLVM runtime dependency. */
#include <errno.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <unistd.h>

static int output(const char *bytes, size_t length) {
    /* Broken pipes are reported as EX_IOERR, not an asynchronous SIGPIPE exit. */
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR) return 74;
    while (length != 0) {
        ssize_t done = write(STDOUT_FILENO, bytes, length);
        if (done < 0 && errno == EINTR) continue;
        if (done <= 0) return 74;
        bytes += (size_t)done;
        length -= (size_t)done;
    }
    return 0;
}
int __oxid_print_bool(int32_t value) {
    return value ? output("true\n", 5) : output("false\n", 6);
}
int __oxid_print_unit(void) { return output("()\n", 3); }
int __oxid_print_i32(int32_t value) {
    char buffer[12]; /* minus, ten digits, newline */
    size_t index = sizeof buffer;
    buffer[--index] = '\n';
    /* Widen before negation, including INT32_MIN. */
    uint32_t magnitude = value < 0 ? (uint32_t)(-(int64_t)value) : (uint32_t)value;
    do {
        buffer[--index] = (char)('0' + magnitude % 10);
        magnitude /= 10;
    } while (magnitude != 0);
    if (value < 0) buffer[--index] = '-';
    return output(buffer + index, sizeof buffer - index);
}
