/* Actual MSVC CRT/Win32 File contracts, independent of generated LLVM text. */
#define OXID_BYTEBUF_TESTING 1
#include <assert.h>
#include <errno.h>
#include <io.h>
#include <stdio.h>
#include <stdint.h>
static int scripted_fd = -1, interrupt_read, short_write, interrupt_close;
static int read_attempts, write_attempts, close_attempts;
static int attempt_read(int fd, void *bytes, unsigned int length);
static int attempt_write(int fd, const void *bytes, unsigned int length);
static int attempt_close(int fd);
#define _read attempt_read
#define _write attempt_write
#define _close attempt_close
#include "../native/typed_bytebuf.c"
#undef _read
#undef _write
#undef _close
#include <winioctl.h>

static int attempt_read(int fd, void *bytes, unsigned int length) {
    if (fd == scripted_fd) {
        ++read_attempts;
        if (interrupt_read) { interrupt_read = 0; errno = EINTR; return -1; }
    }
    return _read(fd, bytes, length);
}
static int attempt_write(int fd, const void *bytes, unsigned int length) {
    if (fd == scripted_fd) {
        ++write_attempts;
        if (short_write && length > 2) length = 2;
    }
    return _write(fd, bytes, length);
}
static int attempt_close(int fd) {
    if (fd == scripted_fd) {
        ++close_attempts;
        if (interrupt_close) {
            interrupt_close = 0;
            assert(_close(fd) == 0);
            errno = EINTR;
            return -1;
        }
    }
    return _close(fd);
}

static uint64_t fuel = UINT64_C(10000000);
static const uint8_t path[] = "file-\xE8\xB3\x87\xE6\x96\x99-\xF0\x9F\x98\x80.bin";
static FILE *fixture_file(const wchar_t *name, const wchar_t *mode) {
    FILE *file = NULL;
    assert(_wfopen_s(&file, name, mode) == 0 && file);
    return file;
}
static oxid_file opened(const uint8_t *name, int length, int create) {
    oxid_file_open_result result;
    assert(__oxid_file_delivery(create, NULL, (uint8_t *)name, length, &fuel, &result, 0) == 0);
    assert(result.tag == 0);
    return result.value.file;
}
static oxid_binary_result operation(int number, oxid_file *file, uint8_t *bytes, int length) {
    oxid_binary_result result;
    assert(__oxid_file_delivery(number, file, bytes, length, &fuel, &result, 0) == 0);
    return result;
}
static HANDLE handle(oxid_file *file) {
    oxid_allocation *node = NULL;
    assert(locate_file(&execution_heap, file, &node) == 0 && node);
    return (HANDLE)_get_osfhandle((int)node->initialized);
}
static void close_checked(oxid_file *file) {
    HANDLE old = handle(file);
    oxid_binary_result result = operation(5, file, NULL, 0);
    assert(result.tag == 0 && file->identity == 0);
    DWORD flags;
    assert(!GetHandleInformation(old, &flags) && GetLastError() == ERROR_INVALID_HANDLE);
    assert(operation(5, file, NULL, 0).tag == 0);
}
static void roundtrip(void) {
    uint8_t expected[256], bytes[256];
    for (unsigned int i = 0; i < 256; ++i) expected[i] = (uint8_t)i;
    oxid_file output = opened(path, (int)sizeof(path) - 1, 1);
    oxid_bytebuf wrong_class = {output.identity, 0, 1};
    assert(__oxid_bytebuf_valid(&wrong_class) == OXID_BUFFER_DESCRIPTOR);
    DWORD flags;
    assert(GetHandleInformation(handle(&output), &flags) && !(flags & HANDLE_FLAG_INHERIT));
    oxid_binary_result result = operation(3, &output, expected, 256);
    assert(result.tag == 0 && result.value == 256);
    assert(operation(4, &output, NULL, 0).tag == 0);
    close_checked(&output);
    assert(__oxid_file_valid(&output) == 0);
    result = operation(3, &output, expected, 1);
    assert(result.tag == 1 && result.value == EBADF);
    oxid_file input = opened(path, (int)sizeof(path) - 1, 0);
    memset(bytes, 0xcc, sizeof bytes);
    result = operation(2, &input, bytes, 3);
    assert(result.tag == 0 && result.value == 3 && memcmp(bytes, expected, 3) == 0);
    for (unsigned int i = 3; i < sizeof bytes; ++i) assert(bytes[i] == 0xcc);
    result = operation(2, &input, bytes, 256);
    assert(result.tag == 0 && result.value == 253 && memcmp(bytes, expected + 3, 253) == 0);
    for (unsigned int i = 253; i < sizeof bytes; ++i) assert(bytes[i] == 0xcc);
    SetLastError(12345);
    result = operation(3, &input, expected, 1);
    assert(result.tag == 1 && result.value == EBADF);
    assert(__oxid_file_seek_absolute(&input, 0, &fuel, &result, 0) == 0 && result.tag == 0);
    result = operation(2, &input, bytes, 256);
    assert(result.tag == 0 && result.value == 256 && memcmp(bytes, expected, 256) == 0);
    result = operation(2, &input, bytes, 1);
    assert(result.tag == 1);
    result = operation(2, &input, bytes, 0);
    assert(result.tag == 0 && result.value == 0);
    close_checked(&input);
    assert(_wremove(L"file-\u8CC7\u6599-\U0001F600.bin") == 0);
}
static void failures(void) {
    for (int boundary = ALLOCATE; boundary <= COMMIT; ++boundary) {
        FILE *file = fixture_file(L"keep.bin", L"wb");
        assert(file && fwrite("keep", 1, 4, file) == 4 && fclose(file) == 0);
        execution_heap.fault_boundary = boundary;
        execution_heap.fault_skip = 0;
        oxid_file_open_result result;
        assert(__oxid_file_delivery(1, NULL, (uint8_t *)"keep.bin", 8, &fuel, &result, 0) == 0);
        assert(result.tag == 3 && !execution_heap.head && !execution_heap.ledger.live_bytes);
        file = fixture_file(L"keep.bin", L"rb");
        char bytes[4];
        assert(file && fread(bytes, 1, 4, file) == 4 && memcmp(bytes, "keep", 4) == 0 && fclose(file) == 0);
    }
    for (int bad = 0; bad < 2; ++bad) {
        uint8_t invalid[] = {'k','e','e','p','.','b','i','n',0,'x'};
        if (bad) { invalid[8] = 0xc0; invalid[9] = 0xaf; }
        oxid_file_open_result result;
        assert(__oxid_file_delivery(1, NULL, invalid, 10, &fuel, &result, 0) == 0);
        assert(result.tag == 1 && result.value.error == EINVAL);
        assert(!execution_heap.head);
    }
    FILE *file = fixture_file(L"keep.bin", L"rb");
    char bytes[4];
    assert(file && fread(bytes, 1, 4, file) == 4 && memcmp(bytes, "keep", 4) == 0 && fclose(file) == 0);
    assert(_wremove(L"keep.bin") == 0);
    oxid_file owned = opened((const uint8_t *)"cleanup.bin", 11, 1);
    HANDLE old = handle(&owned);
    oxid_file stale = owned;
    oxid_heap_cleanup(&execution_heap);
    DWORD flags;
    assert(!GetHandleInformation(old, &flags));
    assert(__oxid_file_valid(&stale) == OXID_BUFFER_DESCRIPTOR);
    assert(execution_heap.ledger.live_bytes == 0 && execution_heap.ledger.live_allocations == 0);
    assert(execution_heap.ledger.allocations == execution_heap.ledger.releases);
    assert(_wremove(L"cleanup.bin") == 0);
}
static void one_attempt_contract(void) {
    oxid_file output = opened((const uint8_t *)"short.bin", 9, 1);
    oxid_allocation *node = NULL;
    assert(locate_file(&execution_heap, &output, &node) == 0 && node);
    scripted_fd = (int)node->initialized;
    short_write = 1;
    uint8_t bytes[] = {'A', 'B', 'C', 'D'};
    oxid_binary_result result = operation(3, &output, bytes, 4);
    assert(result.tag == 0 && result.value == 2 && write_attempts == 1);
    short_write = 0;
    interrupt_close = 1;
    result = operation(5, &output, NULL, 0);
    assert(result.tag == 1 && result.value == EINTR && output.identity == 0 && close_attempts == 1);
    assert(operation(5, &output, NULL, 0).tag == 0 && close_attempts == 1);
    scripted_fd = -1;
    oxid_file input = opened((const uint8_t *)"short.bin", 9, 0);
    assert(locate_file(&execution_heap, &input, &node) == 0 && node);
    scripted_fd = (int)node->initialized;
    interrupt_read = 1;
    result = operation(2, &input, bytes, 4);
    assert(result.tag == 2 && result.value == EINTR && read_attempts == 1);
    result = operation(2, &input, bytes, 4);
    assert(result.tag == 0 && result.value == 2 && bytes[0] == 'A' && bytes[1] == 'B' && read_attempts == 2);
    scripted_fd = -1;
    close_checked(&input);
    assert(_wremove(L"short.bin") == 0);
}
static void large_seek(void) {
    oxid_file file = opened((const uint8_t *)"sparse.bin", 10, 1);
    DWORD returned;
    assert(DeviceIoControl(handle(&file), FSCTL_SET_SPARSE, NULL, 0, NULL, 0, &returned, NULL));
    uint8_t first = 'A', last = 'Z';
    oxid_binary_result result;
    result = operation(3, &file, &first, 1);
    assert(result.tag == 0 && result.value == 1);
    assert(__oxid_file_seek_absolute(&file, INT64_C(4294967419), &fuel, &result, 0) == 0 && result.tag == 0);
    result = operation(3, &file, &last, 1);
    assert(result.tag == 0 && result.value == 1);
    close_checked(&file);
    file = opened((const uint8_t *)"sparse.bin", 10, 0);
    assert(__oxid_file_seek_absolute(&file, -1, &fuel, &result, 0) == 0 && result.tag == 1 && result.value == EINVAL);
    uint8_t byte = 0;
    result = operation(2, &file, &byte, 1);
    assert(result.tag == 0 && result.value == 1 && byte == first);
    assert(__oxid_file_seek_absolute(&file, INT64_C(4294967419), &fuel, &result, 0) == 0 && result.tag == 0);
    result = operation(2, &file, &byte, 1);
    assert(result.tag == 0 && result.value == 1 && byte == last);
    close_checked(&file);
    assert(_wremove(L"sparse.bin") == 0);
}
int main(void) {
    oxid_heap_init(&execution_heap);
    roundtrip(); failures(); one_attempt_contract(); large_seek();
    assert(!execution_heap.head && execution_heap.ledger.allocations == execution_heap.ledger.releases);
    return 0;
}
