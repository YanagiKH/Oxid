#include "../../native/typed_preview.c"
#include "../../native/typed_character.c"
#include <string.h>

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

static int32_t scalar_argument_count;
static char **scalar_argument_values;

void __oxid_scalar_arguments_start(int32_t count, char **values) {
    scalar_argument_count = count;
    scalar_argument_values = values;
}

uint64_t __oxid_scalar_arg_bytes(int32_t index, uint8_t *bytes, int32_t capacity) {
    if (index < 0 || index >= scalar_argument_count || !scalar_argument_values) return 2;
    const char *value = scalar_argument_values[index];
    if (!value) return 2;
    if (capacity < 0 || (capacity && !bytes)) return 3;
    const size_t length = strlen(value);
    if (length > INT32_MAX) return 3;
    const uint64_t payload = (uint64_t)length << 32;
    if (length > (size_t)capacity) return payload | 1;
    if (length) memcpy(bytes, value, length);
    return payload;
}

#ifdef OXID_SCALAR_BYTEBUF
#include "../../native/typed_bytebuf.c"

static _Noreturn void invalid_byte_buffer(int32_t status) {
    __oxid_bytebuf_cleanup();
    if (status == OXID_BUFFER_BOUNDS) {
        static const char message[] = "byte buffer index or range is out of bounds\n";
        __oxid_overflow(message, sizeof(message) - 1);
    }
    static const char message[] = "invalid byte buffer ownership\n";
    __oxid_overflow(message, sizeof(message) - 1);
}

int32_t __oxid_scalar_bytebuf_operation(int32_t operation, oxid_bytebuf *buffer,
    int32_t size, int32_t fill, const uint8_t *source, int32_t length) {
    union { uint64_t wide; int32_t tag; } result;
    uint64_t budget = 0;
    int32_t status = __oxid_bytebuf_operation_hosted(operation, buffer, size, fill,
        source, length, &budget, &result);
    if (status != 0) invalid_byte_buffer(status);
    if (operation == 1 || operation == 2) return (int32_t)result.wide;
    return operation == 7 ? 0 : result.tag;
}

uint8_t *__oxid_scalar_bytebuf_view(const oxid_bytebuf *value, int32_t *length) {
    uint8_t *bytes;
    int32_t status = __oxid_bytebuf_view(value, 0, 0, 0, &bytes, length);
    if (status != 0) invalid_byte_buffer(status);
    return bytes;
}

void __oxid_scalar_bytebuf_finish(void) {
    int32_t status = __oxid_bytebuf_finish();
    if (status != 0) invalid_byte_buffer(status);
}
#endif
