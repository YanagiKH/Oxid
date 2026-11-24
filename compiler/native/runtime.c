#include "../../native/typed_preview.c"
#include "../../native/typed_character.c"

/* The low word selects the status variant. The high word holds its i32 value. */
uint64_t __oxid_scalar_binary_io(int32_t operation, uint8_t *bytes, int32_t length) {
    const uint32_t error_tag = operation == 0 ? 2 : 1;
    if (operation < 0 || operation > 2 || length < 0 || (length && !bytes)) {
        return ((uint64_t)EINVAL << 32) | error_tag;
    }
    if (!length) return 0;
    oxid_io_count transferred;
    if (operation == 0) {
        transferred = native_read(STDIN_FILENO, bytes, (size_t)length);
    } else {
        const int fd = operation == 1 ? STDOUT_FILENO : STDERR_FILENO;
        transferred = native_write(fd, bytes, (size_t)length);
    }
    if (transferred < 0) return ((uint64_t)(uint32_t)errno << 32) | error_tag;
    if (operation == 0 && !transferred) return 1;
    return (uint64_t)(uint32_t)transferred << 32;
}
