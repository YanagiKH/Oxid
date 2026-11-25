#include "../../native/typed_preview.c"
#include "../../native/typed_character.c"
#include <string.h>

#ifdef OXID_SCALAR_TIME
#define OXID_TIME_ONLY
#define OXID_MONOTONIC_MILLIS
#define OXID_UNIX_MILLIS
#define OXID_SLEEP_MILLIS
#include "../../native/typed_termination.c"
#endif

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

#ifdef OXID_SCALAR_HEAP
#include "../../native/typed_bytebuf.c"

static _Noreturn void invalid_heap_resource(int32_t status) {
    __oxid_bytebuf_cleanup();
    if (status == OXID_BUFFER_BOUNDS) {
        static const char message[] = "byte buffer index or range is out of bounds\n";
        __oxid_overflow(message, sizeof(message) - 1);
    }
    static const char message[] = "invalid resource ownership\n";
    __oxid_overflow(message, sizeof(message) - 1);
}

int32_t __oxid_scalar_bytebuf_operation(int32_t operation, oxid_bytebuf *buffer,
    int32_t size, int32_t fill, const uint8_t *source, int32_t length) {
    union { uint64_t wide; int32_t tag; } result;
    uint64_t budget = 0;
    int32_t status = __oxid_bytebuf_operation_hosted(operation, buffer, size, fill,
        source, length, &budget, &result);
    if (status != 0) invalid_heap_resource(status);
    if (operation == 1 || operation == 2) return (int32_t)result.wide;
    return operation == 7 ? 0 : result.tag;
}

uint8_t *__oxid_scalar_bytebuf_view(const oxid_bytebuf *value, int32_t *length) {
    uint8_t *bytes;
    int32_t status = __oxid_bytebuf_view(value, 0, 0, 0, &bytes, length);
    if (status != 0) invalid_heap_resource(status);
    return bytes;
}

void __oxid_scalar_heap_finish(void) {
    int32_t status = __oxid_bytebuf_finish();
    if (status != 0) invalid_heap_resource(status);
}

int32_t __oxid_scalar_file_open(int32_t operation, uint8_t *path, int32_t length,
    oxid_file *file, int32_t *error) {
    oxid_file_open_result opened = {0};
    uint64_t budget = 0;
    int32_t status = __oxid_file_delivery_hosted(operation, NULL, path, length, &budget, &opened, 0);
    if (status != 0) invalid_heap_resource(status);
    *file = (oxid_file){0, 0};
    *error = 0;
    if (opened.tag == 0) *file = opened.value.file;
    if (opened.tag == 1) *error = opened.value.error;
    return opened.tag;
}

uint64_t __oxid_scalar_file_io(int32_t operation, oxid_file *file, uint8_t *bytes, int32_t length) {
    oxid_binary_result result = {0};
    uint64_t budget = 0;
    int32_t status = __oxid_file_delivery_hosted(operation, file, bytes, length, &budget, &result, 0);
    if (status != 0) invalid_heap_resource(status);
    return ((uint64_t)(uint32_t)result.value << 32) | (uint32_t)result.tag;
}

uint64_t __oxid_scalar_file_seek(const oxid_file *file, int64_t offset) {
    oxid_binary_result result = {0};
    uint64_t budget = 0;
    int32_t status = __oxid_file_seek_absolute_hosted(file, offset, &budget, &result, 0);
    if (status != 0) invalid_heap_resource(status);
    return ((uint64_t)(uint32_t)result.value << 32) | (uint32_t)result.tag;
}

int32_t __oxid_scalar_vector_operation(int32_t operation, uint32_t element,
    oxid_vector *vector, int32_t count, const uint8_t *scalar,
    const uint8_t *source, int32_t source_count) {
    union { uint64_t wide; int32_t tag; } result;
    uint64_t budget = 0;
    int32_t status = __oxid_vector_operation_hosted(operation, element, vector, count, scalar,
        source, source_count, &budget, &result, 0);
    if (status != 0) invalid_heap_resource(status);
    if (operation == 1 || operation == 2) return (int32_t)result.wide;
    return operation == 7 ? 0 : result.tag;
}

uint8_t *__oxid_scalar_vector_index(const oxid_vector *vector, uint32_t element, int32_t index) {
    uint8_t *bytes;
    int32_t status = __oxid_vector_index(vector, element, index, &bytes);
    if (status == OXID_BUFFER_BOUNDS) {
        static const char message[] = "vector index out of bounds\n";
        __oxid_bytebuf_cleanup();
        __oxid_overflow(message, sizeof(message) - 1);
    }
    if (status != 0) invalid_heap_resource(status);
    return bytes;
}

uint8_t *__oxid_scalar_vector_view(const oxid_vector *vector, uint32_t element, int32_t *length) {
    uint8_t *bytes;
    int32_t status = __oxid_vector_view(vector, element, 0, 0, 0, &bytes, length);
    if (status != 0) invalid_heap_resource(status);
    return bytes;
}

void __oxid_scalar_vector_pack(uint32_t element, uint8_t *owner, uint8_t *packed) {
    const oxid_vector_record_schema *schema = vector_record_schema(element);
    if (!schema || !vector_record_layout_valid(schema) || !owner
        || vector_heap_overlap(owner, schema->owner_bytes)) {
        invalid_heap_resource(OXID_BUFFER_DESCRIPTOR);
    }
    /* LLVM bool fields use i1. Normalize their containing bytes in the
       temporary owner before the dense runtime validates and copies them. */
    for (uint32_t i = 0; i < schema->leaf_count; ++i) {
        const oxid_vector_record_leaf *leaf = &schema->leaves[i];
        if (leaf->element == OXID_VECTOR_BOOL) owner[leaf->owner_offset] &= 1;
    }
    int32_t status = __oxid_record_vector_pack(element, owner, packed);
    if (status != 0) invalid_heap_resource(status);
}

void __oxid_scalar_vector_unpack(uint32_t element, const uint8_t *packed, uint8_t *owner) {
    int32_t status = __oxid_record_vector_unpack(element, packed, owner);
    if (status != 0) invalid_heap_resource(status);
}

void __oxid_scalar_vector_write(uint32_t element, const uint8_t *packed, uint8_t *destination) {
    int32_t status = __oxid_record_vector_write(element, packed, destination);
    if (status != 0) invalid_heap_resource(status);
}
#endif
