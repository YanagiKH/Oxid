/* Internal raw Win32 runtime adapter qualification; process-mode source remains closed. */
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <stdint.h>
#include <string.h>

extern int32_t __oxid_read_stdin_byte(uint8_t *byte);
extern int32_t __oxid_write_stdout_byte(const uint8_t *byte);
extern int32_t __oxid_process_setup(void);
extern _Noreturn void __oxid_process_failure(const char *bytes, uint64_t length);
extern int __oxid_print_i32(int32_t value);
extern int __oxid_print_bool(int32_t value);
extern int __oxid_print_unit(void);
static const uint8_t bytes[] = {0, 13, 10, 26, 255};
int main(int argc, char **argv) {
    if (argc != 2 || __oxid_process_setup() != 0) return 90;
    if (!strcmp(argv[1], "raw")) {
        for (unsigned int i = 0; i < sizeof bytes; ++i) {
            if (__oxid_write_stdout_byte(bytes + i) != 1) return 91;
        }
        __oxid_process_failure((const char *)bytes, sizeof bytes);
    }
    if (!strcmp(argv[1], "read")) {
        uint8_t byte = 0;
        for (unsigned int i = 0; i < sizeof bytes; ++i) {
            if (__oxid_read_stdin_byte(&byte) != 1 || byte != bytes[i]) return 92;
        }
        return __oxid_read_stdin_byte(&byte) == 0 ? 0 : 93;
    }
    if (!strcmp(argv[1], "broken-pipe")) {
        HANDLE reader, writer;
        if (!CreatePipe(&reader, &writer, NULL, 0)) return 94;
        CloseHandle(reader);
        if (!SetStdHandle(STD_OUTPUT_HANDLE, writer)) return 95;
        int32_t status = __oxid_write_stdout_byte(bytes);
        CloseHandle(writer);
        return status == -2 ? 0 : 96;
    }
    if (!strcmp(argv[1], "print")) {
        if (__oxid_print_i32(INT32_MIN) || __oxid_print_i32(INT32_MAX)
            || __oxid_print_bool(1) || __oxid_print_bool(0) || __oxid_print_unit()) return 97;
        return 0;
    }
    return 98;
}
