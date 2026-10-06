/* Experimental Linux scalar output adapter. No Oxid/LLVM runtime dependency. */
#include <errno.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <unistd.h>

/* Private bounded-input adapter. Exactly one unbuffered one-byte attempt;
 * retries and their fuel belong to the verified caller. Never change stdin,
 * process signal policy, or descriptor flags here. The byte is valid only on 1.
 */
int32_t __oxid_read_stdin_byte(uint8_t *byte) {
    ssize_t done = read(STDIN_FILENO, byte, 1);
    if (done == 1) return 1;
    if (done == 0) return 0;
    return errno == EINTR ? -1 : -2;
}

static int output(int fd, const char *bytes, size_t length) {
    /* Broken pipes are reported as EX_IOERR, not an asynchronous SIGPIPE exit. */
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR) return 74;
    while (length != 0) {
        ssize_t done = write(fd, bytes, length);
        if (done < 0 && errno == EINTR) continue;
        if (done <= 0) return 74;
        bytes += (size_t)done;
        length -= (size_t)done;
    }
    return 0;
}
int __oxid_print_bool(int32_t value) {
    return value ? output(STDOUT_FILENO, "true\n", 5) : output(STDOUT_FILENO, "false\n", 6);
}
int __oxid_print_unit(void) { return output(STDOUT_FILENO, "()\n", 3); }
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
    return output(STDOUT_FILENO, buffer + index, sizeof buffer - index);
}

/* Private Linux x86_64 ABI: i64 is the byte length, not a NUL-terminated string.
 * No result was printed before this first-error path. Never return to Oxid code.
 */
_Noreturn void __oxid_overflow(const char *message, uint64_t length) {
    int status = output(STDERR_FILENO, message, (size_t)length);
    _exit(status == 0 ? 1 : status);
}
