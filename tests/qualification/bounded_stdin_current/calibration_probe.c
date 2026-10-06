/* Observation-only probe; the controller supplies independent literal results. */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>

static void observe(const char *label, int fd, size_t count) {
    unsigned char buffer[2] = {0xa5, 0x5a};
    errno = 0;
    const ssize_t result = read(fd, buffer, count);
    const int read_errno = errno;
    printf("%s count=%zu result=%zd errno=%d buffer=%02x%02x\n",
           label, count, result, read_errno, buffer[0], buffer[1]);
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    const int other_fd = open(argv[1], O_RDONLY);
    if (other_fd < 3) return 3;
    observe("other-1", other_fd, 1);
    observe("stdin-zero-before", STDIN_FILENO, 0);
    observe("stdin-two", STDIN_FILENO, 2);
    observe("stdin-one-1", STDIN_FILENO, 1);
    observe("other-2", other_fd, 1);
    observe("stdin-zero-between", STDIN_FILENO, 0);
    observe("stdin-one-2", STDIN_FILENO, 1);
    observe("other-3", other_fd, 1);
    observe("stdin-one-3", STDIN_FILENO, 1);
    observe("stdin-one-4", STDIN_FILENO, 1);
    observe("stdin-one-5", STDIN_FILENO, 1);
    observe("stdin-one-6", STDIN_FILENO, 1);
    observe("other-eof", other_fd, 1);
    observe("other-invalid", -1, 1);
    return close(other_fd) == 0 ? 0 : 4;
}
