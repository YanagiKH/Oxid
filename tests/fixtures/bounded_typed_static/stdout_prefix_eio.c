#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>

/* Local validation injection only. No input, stderr, signal, or global hooks. */
ssize_t write(int fd, const void *buffer, size_t count) {
    static ssize_t (*real_write)(int, const void *, size_t);
    static size_t emitted;
    if (!real_write) real_write = dlsym(RTLD_NEXT, "write");
    if (!real_write) { errno = EIO; return -1; }
    const char *limit = getenv("OXID_STDOUT_PREFIX_BYTES");
    const char *device = getenv("OXID_STDOUT_TARGET_DEV");
    const char *inode = getenv("OXID_STDOUT_TARGET_INO");
    struct stat st;
    if (fd != STDOUT_FILENO || !limit || !device || !inode ||
        fstat(fd, &st) != 0 ||
        (uintmax_t)st.st_dev != strtoull(device, 0, 10) ||
        (uintmax_t)st.st_ino != strtoull(inode, 0, 10))
        return real_write(fd, buffer, count);
    if (count == 0) return real_write(fd, buffer, count);
    size_t budget = (size_t)strtoull(limit, 0, 10);
    if (emitted >= budget) { errno = EIO; return -1; }
    if (count > budget - emitted) count = budget - emitted;
    ssize_t result = real_write(fd, buffer, count);
    if (result > 0) emitted += (size_t)result;
    return result;
}
