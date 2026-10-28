/* Experimental Linux scalar output adapter. No Oxid/LLVM runtime dependency. */
#include <errno.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
_Static_assert(sizeof(void *) == 8 && sizeof(size_t) == 8 && sizeof(long) == 4,
               "Windows native requires the MSVC x64 LLP64 ABI");
typedef intptr_t oxid_io_count;
#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2
static HANDLE standard_handle(int fd) {
    return fd == 0 ? GetStdHandle(STD_INPUT_HANDLE)
        : fd == 1 ? GetStdHandle(STD_OUTPUT_HANDLE)
        : fd == 2 ? GetStdHandle(STD_ERROR_HANDLE) : INVALID_HANDLE_VALUE;
}
static oxid_io_count native_read(int fd, void *bytes, size_t length) {
    HANDLE handle = standard_handle(fd);
    if (handle == NULL || handle == INVALID_HANDLE_VALUE || length > UINT32_MAX) return -1;
    DWORD count = 0;
    if (ReadFile(handle, bytes, (DWORD)length, &count, NULL)) return (oxid_io_count)count;
    return GetLastError() == ERROR_BROKEN_PIPE ? 0 : -1;
}
static oxid_io_count native_write(int fd, const void *bytes, size_t length) {
    HANDLE handle = standard_handle(fd);
    if (handle == NULL || handle == INVALID_HANDLE_VALUE || length > UINT32_MAX) return -1;
    DWORD count = 0;
    return WriteFile(handle, bytes, (DWORD)length, &count, NULL) ? (oxid_io_count)count : -1;
}
static int native_interrupted(void) { return 0; }
static _Noreturn void native_exit(int status) { ExitProcess((UINT)status); }
#else
#include <unistd.h>
typedef ssize_t oxid_io_count;
#define native_read read
#define native_write write
#define native_exit _exit
static int native_interrupted(void) { return errno == EINTR; }
#endif

/* Private bounded-input adapter. Exactly one unbuffered one-byte attempt;
 * retries and their fuel belong to the verified caller. Never change stdin,
 * process signal policy, or descriptor flags here. The byte is valid only on 1.
 */
int32_t __oxid_read_stdin_byte(uint8_t *byte) {
    oxid_io_count done = native_read(STDIN_FILENO, byte, 1);
    if (done == 1) return 1;
    if (done == 0) return 0;
    return native_interrupted() ? -1 : -2;
}

/* Private process-mode setup. Call before any diagnostic or source effect.
 * Failure is handled by a silent status 74 exit, never another write. */
int32_t __oxid_process_setup(void) {
#if defined(_WIN32)
    /* Raw handle I/O never enters CRT text translation or changes console/FD modes. */
    return 0;
#else
    return signal(SIGPIPE, SIG_IGN) == SIG_ERR ? -1 : 0;
#endif
}

/* Exactly one attempt. The verified caller stages and validates the byte,
 * counts partial progress, and pays for every retry before calling again. */
int32_t __oxid_write_stdout_byte(const uint8_t *byte) {
    oxid_io_count done = native_write(STDOUT_FILENO, byte, 1);
    if (done == 1) return 1;
    if (done < 0 && native_interrupted()) return -1;
    return -2;
}

/* Setup has already succeeded. This terminal diagnostic never writes stdout,
 * changes signal policy again, or recursively reports a reporting failure. */
_Noreturn void __oxid_process_failure(const char *message, uint64_t length) {
    while (length != 0) {
        oxid_io_count done = native_write(STDERR_FILENO, message, (size_t)length);
        if (done < 0 && native_interrupted()) continue;
        if (done <= 0 || (uint64_t)done > length) native_exit(74);
        message += (size_t)done;
        length -= (uint64_t)done;
    }
    native_exit(1);
}

static int output(int fd, const char *bytes, size_t length) {
    /* Broken pipes are reported as EX_IOERR, not an asynchronous SIGPIPE exit. */
#if !defined(_WIN32)
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR) return 74;
#endif
    while (length != 0) {
        oxid_io_count done = native_write(fd, bytes, length);
        if (done < 0 && native_interrupted()) continue;
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

int __oxid_print_i64(int64_t value) {
    char buffer[21]; /* minus, nineteen digits, newline */
    size_t index = sizeof buffer;
    buffer[--index] = '\n';
    /* Unsigned subtraction includes INT64_MIN without signed overflow. */
    uint64_t magnitude = value < 0 ? UINT64_C(0) - (uint64_t)value : (uint64_t)value;
    do {
        buffer[--index] = (char)('0' + magnitude % 10);
        magnitude /= 10;
    } while (magnitude != 0);
    if (value < 0) buffer[--index] = '-';
    return output(STDOUT_FILENO, buffer + index, sizeof buffer - index);
}

int __oxid_print_u64(uint64_t value) {
    char buffer[21]; /* twenty decimal digits and newline */
    size_t index = sizeof buffer;
    buffer[--index] = '\n';
    do {
        buffer[--index] = (char)('0' + value % 10);
        value /= 10;
    } while (value != 0);
    return output(STDOUT_FILENO, buffer + index, sizeof buffer - index);
}

/* Private Linux x86_64 ABI: i64 is the byte length, not a NUL-terminated string.
 * No result was printed before this first-error path. Never return to Oxid code.
 */
_Noreturn void __oxid_overflow(const char *message, uint64_t length) {
    int status = output(STDERR_FILENO, message, (size_t)length);
    native_exit(status == 0 ? 1 : status);
}
