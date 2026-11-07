/* Real Windows executables for adversarial public doctor/tool failure controls. */
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <process.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>

static int emit(DWORD stream, const void *data, DWORD length) {
    const uint8_t *bytes = data;
    while (length) {
        DWORD done = 0;
        if (!WriteFile(GetStdHandle(stream), bytes, length, &done, NULL) || !done) return 92;
        bytes += done;
        length -= done;
    }
    return 0;
}
static int environment(const wchar_t *key, wchar_t *buffer, DWORD length) {
    DWORD count = GetEnvironmentVariableW(key, buffer, length);
    return count && count < length;
}
static int marker(const wchar_t *name) {
    wchar_t directory[32768], path[32768];
    if (!environment(L"OXID_NATIVE_PROBE_STATE", directory, 32768)) return 93;
    if (swprintf(path, 32768, L"%ls\\%ls", directory, name) < 0) return 93;
    HANDLE file = CreateFileW(path, GENERIC_WRITE, FILE_SHARE_READ, NULL, CREATE_NEW,
                              FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return 93;
    char pid[32];
    int count = snprintf(pid, sizeof pid, "%lu", (unsigned long)GetCurrentProcessId());
    DWORD done = 0;
    int okay = count > 0 && WriteFile(file, pid, (DWORD)count, &done, NULL) && done == (DWORD)count;
    CloseHandle(file);
    return okay ? 0 : 93;
}
static int descendant(void) {
    if (marker(L"child.pid")) return 93;
    Sleep(30000);
    return marker(L"escaped-child");
}
static int spawn_descendant(void) {
    wchar_t self[32768], command[32768];
    DWORD length = GetModuleFileNameW(NULL, self, 32768);
    if (!length || length >= 32768 || swprintf(command, 32768, L"\"%ls\" --descendant", self) < 0) return 93;
    STARTUPINFOW startup = {0};
    PROCESS_INFORMATION process = {0};
    startup.cb = sizeof startup;
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    startup.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
    startup.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    if (!CreateProcessW(self, command, NULL, NULL, TRUE, CREATE_NO_WINDOW, NULL, NULL, &startup, &process)) return 93;
    CloseHandle(process.hThread);
    CloseHandle(process.hProcess);
    /* Handshake independently exposes a live child before the tested parent
     * exits or exceeds its deadline. The production job owns it from birth. */
    for (unsigned int retry = 0; retry < 200; ++retry) {
        wchar_t directory[32768], path[32768];
        if (!environment(L"OXID_NATIVE_PROBE_STATE", directory, 32768)
            || swprintf(path, 32768, L"%ls\\child.pid", directory) < 0) return 93;
        if (GetFileAttributesW(path) != INVALID_FILE_ATTRIBUTES) {
            if (swprintf(path, 32768, L"%ls\\observer.ready", directory) < 0) return 93;
            if (GetFileAttributesW(path) != INVALID_FILE_ATTRIBUTES) return 0;
        }
        Sleep(5);
    }
    return 93;
}
int wmain(int argc, wchar_t **argv) {
    if (argc == 2 && !wcscmp(argv[1], L"--descendant")) return descendant();
    wchar_t mode[64] = {0}, self[32768], real[32768];
    environment(L"OXID_NATIVE_PROBE_MODE", mode, 64);
    DWORD self_length = GetModuleFileNameW(NULL, self, 32768);
    if (!self_length || self_length >= 32768) return 93;
    const wchar_t *base = wcsrchr(self, L'\\');
    base = base ? base + 1 : self;
    const int clang = !_wcsicmp(base, L"clang.exe");
    const int opt = !_wcsicmp(base, L"opt.exe");
    const int linker = !_wcsicmp(base, L"lld-link.exe");
    if (argc == 2 && !wcscmp(argv[1], L"--version")) {
        if (clang && !wcscmp(mode, L"wrong-version")) return emit(STD_OUTPUT_HANDLE, "clang version 20.0.0\n", 21);
        if (clang && (!wcscmp(mode, L"stdout-flood") || !wcscmp(mode, L"stderr-flood"))) {
            char bytes[4096] = {0};
            DWORD remaining = 65537;
            while (remaining) {
                DWORD count = remaining < sizeof bytes ? remaining : (DWORD)sizeof bytes;
                if (emit(!wcscmp(mode, L"stderr-flood") ? STD_ERROR_HANDLE : STD_OUTPUT_HANDLE, bytes, count)) return 92;
                remaining -= count;
            }
            return 0;
        }
        if (clang && (!wcscmp(mode, L"sleep-tree") || !wcscmp(mode, L"exit-tree"))) {
            int status = spawn_descendant();
            if (status) return status;
            if (!wcscmp(mode, L"sleep-tree")) Sleep(30000);
        }
        const char *version = clang ? "clang version 19.1.7\n" : opt ? "LLVM version 19.1.7\n" : "LLD 19.1.7\n";
        return emit(STD_OUTPUT_HANDLE, version, (DWORD)strlen(version));
    }
    if (!wcscmp(mode, L"link-failure") && linker) {
        if (marker(L"linker.pid")) return 93;
        return emit(STD_ERROR_HANDLE, "controlled lld-link failure\n", 28) ? 92 : 73;
    }
    if (!environment(L"OXID_NATIVE_REAL_CLANG", real, 32768) || !clang) return 94;
    /* The SDK failure control invokes the real compiler with SDK headers
     * deliberately hidden. It does not fabricate a compiler diagnostic. */
    if (argc >= 128) return 95;
    const wchar_t *forward[131];
    int count = 0;
    forward[count++] = real;
    if (!wcscmp(mode, L"missing-sdk-headers")) forward[count++] = L"-nostdinc";
    for (int index = 1; index < argc; ++index) forward[count++] = argv[index];
    forward[count] = NULL;
    intptr_t result = _wspawnv(_P_WAIT, real, forward);
    return result == -1 ? 96 : (int)result;
}
