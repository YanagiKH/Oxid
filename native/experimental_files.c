/* Included after experimental_bytebuf.c: files use the same checked registry,
 * allocation charge and execution lifetime, with a distinct resource class. */
#include <fcntl.h>
/* Strict C11 on Linux can hide this POSIX flag. Darwin declares its own value. */
#ifndef O_CLOEXEC
#if defined(__linux__)
#define O_CLOEXEC 02000000
#else
#error "O_CLOEXEC is required for native file handles"
#endif
#endif

typedef struct { uint64_t identity, reserved; } oxid_file;
typedef struct {
    int32_t tag, padding;
    union { oxid_file file; int32_t error; } value;
} oxid_file_open_result;
_Static_assert(sizeof(oxid_file) == 16, "file identity carrier");
_Static_assert(sizeof(oxid_file_open_result) == 24, "file result carrier");
_Static_assert(offsetof(oxid_file_open_result, value) == 8, "file result payload");

static int32_t locate_file(oxid_byte_heap *heap, const oxid_file *value,
    oxid_allocation **found) {
    if (!heap || !value || !found || value->reserved) return OXID_BUFFER_DESCRIPTOR;
    if (!value->identity) { *found = NULL; return 0; }
    for (oxid_allocation *node = heap->head; node; node = node->next) {
        if (node->identity == value->identity) {
            if (node->capacity != UINT32_MAX || node->initialized > INT32_MAX)
                return OXID_BUFFER_DESCRIPTOR;
            *found = node;
            return 0;
        }
    }
    return OXID_BUFFER_DESCRIPTOR;
}
int32_t __oxid_file_valid(const oxid_file *value) {
    oxid_allocation *node;
    return locate_file(&execution_heap, value, &node);
}
static int32_t close_file(oxid_byte_heap *heap, oxid_file *value, int32_t *error) {
    oxid_allocation *node;
    int32_t status = locate_file(heap, value, &node);
    if (status) return status;
    *error = 0;
    if (node) {
        /* Linux and Darwin release the descriptor before final close errors.
         * Never retry against a descriptor number that may have been reused. */
        if (close((int)node->initialized) < 0) *error = errno;
        node->initialized = UINT32_MAX;
        unlink_node(heap, node);
    }
    *value = (oxid_file){0, 0};
    return 0;
}
void __oxid_file_drop(oxid_file *value) {
    int32_t error;
    (void)close_file(&execution_heap, value, &error);
}
static int overlaps(const void *left, size_t left_size, const void *right, size_t right_size) {
    if (!left_size || !right_size) return 0;
    uintptr_t a = (uintptr_t)left, b = (uintptr_t)right;
    return a <= b ? b - a < left_size : a - b < right_size;
}
static int32_t open_file(oxid_byte_heap *heap, const uint8_t *bytes, int32_t length,
    int create, oxid_file_open_result *result) {
    result->tag = 1;
    if (length >= 4096) { result->value.error = ENAMETOOLONG; return 0; }
    if (length && memchr(bytes, 0, (size_t)length)) { result->value.error = EINVAL; return 0; }
    char path[4096];
    if (length) memcpy(path, bytes, (size_t)length);
    path[length] = 0;
    if (heap->ledger.live_bytes > heap->limit || OXID_BYTEBUF_HEADER > heap->limit - heap->ledger.live_bytes) {
        result->tag = 2; return 0;
    }
    if (heap->next_identity == UINT64_MAX) return OXID_BUFFER_IDENTITY;
    if (fail(heap, ALLOCATE)) { result->tag = 3; return 0; }
    oxid_allocation *node = malloc(sizeof(*node));
    if (!node) { result->tag = 3; return 0; }
    *node = (oxid_allocation){ heap->head, NULL, heap->next_identity, UINT32_MAX, UINT32_MAX };
    if (heap->head) heap->head->previous = node;
    heap->head = node;
    ++heap->next_identity;
    heap->ledger.live_bytes += OXID_BYTEBUF_HEADER;
    ++heap->ledger.live_allocations;
    ++heap->ledger.allocations;
    if (heap->ledger.live_bytes > heap->ledger.peak_bytes)
        heap->ledger.peak_bytes = heap->ledger.live_bytes;
    /* Every injected failure precedes open, including its O_TRUNC side effect. */
    if (fail(heap, INITIALIZE) || fail(heap, COMMIT)) {
        unlink_node(heap, node); result->tag = 3; return 0;
    }
    int flags = O_CLOEXEC | O_NOCTTY | (create ? O_WRONLY | O_CREAT | O_TRUNC : O_RDONLY);
    int fd = open(path, flags, 0600);
    if (fd < 0) {
        int error = errno;
        unlink_node(heap, node); result->value.error = error; return 0;
    }
    /* The pending allocation and complete return were admitted before open. */
    node->initialized = (uint32_t)fd;
    result->tag = 0;
    result->value.file = (oxid_file){node->identity, 0};
    return 0;
}

/* Operation 0=open_read, 1=create, 2=read_some, 3=write_some, 4=sync_all,
 * 5=close. All are private to the independently checked canonical emitter. */
int32_t __oxid_file_delivery(int32_t operation, oxid_file *file, uint8_t *bytes,
    int32_t length, uint64_t *fuel, void *result, uint64_t return_cost) {
    if (operation < 0 || operation > 5 || !fuel || !result) return OXID_BUFFER_DESCRIPTOR;
    if (length < 0) return OXID_BUFFER_BOUNDS;
    if (length && !bytes) return OXID_BUFFER_DESCRIPTOR;
    size_t result_size = operation < 2 ? sizeof(oxid_file_open_result) : sizeof(oxid_binary_result);
    if (overlaps(bytes, (size_t)length, result, result_size) ||
        (operation >= 2 && (overlaps(file, sizeof(*file), result, result_size) ||
            overlaps(file, sizeof(*file), bytes, (size_t)length)))) return OXID_BUFFER_DESCRIPTOR;
    oxid_allocation *node = NULL;
    if (operation >= 2) {
        int32_t status = locate_file(&execution_heap, file, &node);
        if (status) return status;
    }
    uint64_t work = operation < 2 ? (length >= 4096 ? 0 : (uint64_t)length * 3 + 1)
        : operation < 4 ? (uint64_t)length : 0;
    if (*fuel < work || *fuel - work < return_cost) return OXID_BUFFER_FUEL;
    *fuel -= work + return_cost;
    /* No fallible admission, allocation or loan check after the OS attempt. */
    if (operation < 2) {
        return open_file(&execution_heap, bytes, length, operation == 1, result);
    } else {
        oxid_binary_result *status = result;
        status->tag = 0; status->value = 0;
        if (operation == 5) {
            int32_t error = 0;
            (void)close_file(&execution_heap, file, &error);
            if (error) { status->tag = 1; status->value = error; }
        } else if (!node) {
            status->tag = operation == 2 ? 2 : 1; status->value = EBADF;
        } else if (operation == 4) {
            if (fsync((int)node->initialized) < 0) { status->tag = 1; status->value = errno; }
        } else if (length) {
            ssize_t count = operation == 2 ? read((int)node->initialized, bytes, (size_t)length)
                : write((int)node->initialized, bytes, (size_t)length);
            if (count < 0) { status->tag = operation == 2 ? 2 : 1; status->value = errno; }
            else if (!count && operation == 2) status->tag = 1;
            else status->value = (int32_t)count;
        }
    }
    return 0;
}
int32_t __oxid_file_delivery_hosted(int32_t operation, oxid_file *file, uint8_t *bytes,
    int32_t length, uint64_t *fuel, void *result, uint64_t return_cost) {
    if (!fuel) return OXID_BUFFER_DESCRIPTOR;
    uint64_t operation_budget = UINT64_MAX;
    return __oxid_file_delivery(operation, file, bytes, length, &operation_budget, result, return_cost);
}

/* Separate entrypoint preserves every existing file operation and its ABI.
 * The caller has prepared both results and the exclusive loan release. */
_Static_assert(sizeof(off_t) == sizeof(int64_t), "file seeking requires 64-bit off_t");
_Static_assert((off_t)-1 < (off_t)0, "file seeking requires signed off_t");
int32_t __oxid_file_seek_absolute(const oxid_file *file, int64_t offset,
    uint64_t *fuel, oxid_binary_result *result, uint64_t return_cost) {
    if (!file || !fuel || !result ||
        overlaps(file, sizeof(*file), result, sizeof(*result))) return OXID_BUFFER_DESCRIPTOR;
    oxid_allocation *node = NULL;
    int32_t status = locate_file(&execution_heap, file, &node);
    if (status) return status;
    if (*fuel < return_cost) return OXID_BUFFER_FUEL;
    *fuel -= return_cost;
    int error = 0;
    if (!node) error = EBADF;
    else if (offset < 0) error = EINVAL;
    else {
        off_t position = lseek((int)node->initialized, (off_t)offset, SEEK_SET);
        if (position < 0) error = errno;
    }
    /* No allocation, admission or source work can fail after lseek. */
    result->tag = error ? 1 : 0;
    result->value = error;
    return 0;
}
int32_t __oxid_file_seek_absolute_hosted(const oxid_file *file, int64_t offset,
    uint64_t *fuel, oxid_binary_result *result, uint64_t return_cost) {
    if (!fuel) return OXID_BUFFER_DESCRIPTOR;
    uint64_t operation_budget = UINT64_MAX;
    return __oxid_file_seek_absolute(file, offset, &operation_budget, result, return_cost);
}
