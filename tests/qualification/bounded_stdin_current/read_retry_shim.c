/* Validation only: exactly two synthetic EINTR outcomes per thread, not signals.
 * Startup LD_PRELOAD only. Other fd/count pairs go directly to the host syscall.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <stddef.h>
#include <sys/syscall.h>
#include <unistd.h>

static _Thread_local unsigned int stdin_one_byte_attempts
    __attribute__((tls_model("initial-exec")));

__attribute__((visibility("default")))
ssize_t read(int fd, void *buffer, size_t count) {
    if (fd == STDIN_FILENO && count == 1 && stdin_one_byte_attempts < 2) {
        ++stdin_one_byte_attempts;
        errno = EINTR;
        return -1;
    }
    return (ssize_t)syscall(SYS_read, fd, buffer, count);
}
