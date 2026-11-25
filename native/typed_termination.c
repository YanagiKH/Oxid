/* Included for process notifications and clocks.
 * _POSIX_C_SOURCE must be selected before the first runtime system header. */
#include <stdint.h>

#ifndef OXID_TIME_ONLY
#if !defined(OXID_TERMINATION_FORCE_UNSUPPORTED) && \
    (defined(__linux__) || defined(__APPLE__)) && \
    (defined(__x86_64__) || defined(__aarch64__)) && UINTPTR_MAX == UINT64_MAX && \
    defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stddef.h>
#include <time.h>
#include <unistd.h>

_Static_assert(sizeof(struct timespec) == 2 * sizeof(int64_t), "termination wait timespec layout");
_Static_assert(sizeof(struct pollfd) == 8, "stdin readiness poll descriptor layout");

static volatile sig_atomic_t termination_flag;
static int termination_active;
static int termination_enabled;
static struct sigaction termination_previous[2];

static void termination_notify(int signal_number) {
    (void)signal_number;
    termination_flag = 1;
}

static void termination_restore_or_exit(int status) {
    if (status != 0) _exit(74);
}

static int termination_block(sigset_t *previous) {
    sigset_t selected;
    if (sigemptyset(&selected) != 0 || sigaddset(&selected, SIGINT) != 0 ||
        sigaddset(&selected, SIGTERM) != 0) return 0;
    return sigprocmask(SIG_BLOCK, &selected, previous) == 0;
}

int32_t __oxid_termination_scope_start(void) {
    if (termination_active) return -1;
    termination_flag = 0;
    termination_enabled = 0;
    termination_active = 1;
    return 0;
}

#ifndef OXID_TERMINATION_INSTALL
#define OXID_TERMINATION_INSTALL(signal_number, action, previous, step) \
    sigaction((signal_number), (action), (previous))
#endif

int32_t __oxid_enable_termination_notifications(void) {
    if (!termination_active) return 0;
    if (termination_enabled) return 1;
    sigset_t previous_mask;
    if (!termination_block(&previous_mask)) return 0;
    struct sigaction action = {0};
    action.sa_handler = termination_notify;
    if (sigemptyset(&action.sa_mask) != 0) {
        termination_restore_or_exit(sigprocmask(SIG_SETMASK, &previous_mask, NULL));
        return 0;
    }
    termination_flag = 0;
    if (OXID_TERMINATION_INSTALL(SIGINT, &action, &termination_previous[0], 1) != 0) {
        termination_restore_or_exit(sigprocmask(SIG_SETMASK, &previous_mask, NULL));
        return 0;
    }
    if (OXID_TERMINATION_INSTALL(SIGTERM, &action, &termination_previous[1], 2) != 0) {
        termination_restore_or_exit(sigaction(SIGINT, &termination_previous[0], NULL));
        termination_restore_or_exit(sigprocmask(SIG_SETMASK, &previous_mask, NULL));
        return 0;
    }
    termination_enabled = 1;
    termination_restore_or_exit(sigprocmask(SIG_SETMASK, &previous_mask, NULL));
    return 1;
}

int32_t __oxid_termination_requested(void) {
    return termination_active && termination_enabled && termination_flag != 0;
}

int32_t __oxid_wait_for_termination(int32_t timeout_ms) {
    if (timeout_ms < 0 || !termination_active || !termination_enabled) return 0;
    if (__oxid_termination_requested()) return 1;
    if (timeout_ms == 0) return 0;
    const int64_t duration = (int64_t)timeout_ms * 1000000;
    struct timespec started;
    if (clock_gettime(CLOCK_MONOTONIC, &started) != 0) return 0;
    while (!__oxid_termination_requested()) {
        struct timespec now;
        if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec < started.tv_sec) return 0;
        const uint64_t seconds = (uint64_t)now.tv_sec - (uint64_t)started.tv_sec;
        if (seconds > (uint64_t)timeout_ms / 1000 + 1) break;
        const int64_t elapsed = (int64_t)seconds * 1000000000 + now.tv_nsec - started.tv_nsec;
        if (elapsed >= duration) break;
        const int64_t remaining = duration - elapsed;
        struct timespec slice = {0, (long)(remaining < 10000000 ? remaining : 10000000)};
        if (nanosleep(&slice, NULL) != 0 && errno != EINTR) return 0;
    }
    return __oxid_termination_requested();
}

int32_t __oxid_wait_for_stdin(int32_t timeout_ms) {
    if (timeout_ms < 0 || !termination_active || !termination_enabled) return 0;
    struct pollfd input = {STDIN_FILENO, POLLIN, 0};
    struct timespec started;
    if (timeout_ms > 0 && clock_gettime(CLOCK_MONOTONIC, &started) != 0) return 0;
    const int64_t duration = (int64_t)timeout_ms * 1000000;
    int32_t remaining_ms = timeout_ms;
    while (!__oxid_termination_requested()) {
        // POSIX may deliver a process signal to another unblocked thread.
        // Limit each kernel wait so cancellation never depends on EINTR.
        const int wait_ms = remaining_ms < 10 ? remaining_ms : 10;
        const int ready = poll(&input, 1, wait_ms);
        if (__oxid_termination_requested()) return 0;
        if (ready > 0) {
            return !(input.revents & POLLNVAL)
                && (input.revents & (POLLIN | POLLHUP | POLLERR)) != 0;
        }
        if (timeout_ms == 0 || (ready < 0 && errno != EINTR)) return 0;
        struct timespec now;
        if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec < started.tv_sec) return 0;
        const uint64_t seconds = (uint64_t)now.tv_sec - (uint64_t)started.tv_sec;
        if (seconds > (uint64_t)timeout_ms / 1000 + 1) return 0;
        const int64_t elapsed = (int64_t)seconds * 1000000000 + now.tv_nsec - started.tv_nsec;
        if (elapsed < 0 || elapsed >= duration) return 0;
        remaining_ms = (int32_t)((duration - elapsed + 999999) / 1000000);
    }
    return 0;
}

void __oxid_termination_scope_end(void) {
    if (!termination_active) return;
    if (termination_enabled) {
        sigset_t previous_mask;
        if (!termination_block(&previous_mask)) _exit(74);
        termination_restore_or_exit(sigaction(SIGINT, &termination_previous[0], NULL));
        termination_restore_or_exit(sigaction(SIGTERM, &termination_previous[1], NULL));
        termination_enabled = 0;
        termination_restore_or_exit(sigprocmask(SIG_SETMASK, &previous_mask, NULL));
    }
    termination_flag = 0;
    termination_active = 0;
}
#elif !defined(OXID_TERMINATION_FORCE_UNSUPPORTED) && defined(_WIN64) && \
    (defined(_M_X64) || defined(__x86_64__))
#include <windows.h>

enum {
    TERMINATION_ACTIVE = 1,
    TERMINATION_ENABLED = 2,
    TERMINATION_REQUESTED = 4,
};

static volatile LONG termination_state;

static LONG termination_load(void) {
    return InterlockedCompareExchange(&termination_state, 0, 0);
}

static BOOL WINAPI termination_console_handler(DWORD event) {
    if (event != CTRL_C_EVENT && event != CTRL_BREAK_EVENT) return FALSE;
    LONG current = termination_load();
    while (current & TERMINATION_ENABLED) {
        const LONG observed = InterlockedCompareExchange(&termination_state,
            current | TERMINATION_REQUESTED, current);
        if (observed == current) return TRUE;
        current = observed;
    }
    return FALSE;
}

int32_t __oxid_termination_scope_start(void) {
    return InterlockedCompareExchange(&termination_state, TERMINATION_ACTIVE, 0) == 0 ? 0 : -1;
}

int32_t __oxid_enable_termination_notifications(void) {
    const LONG enabled = TERMINATION_ACTIVE | TERMINATION_ENABLED;
    const LONG previous = InterlockedCompareExchange(&termination_state, enabled, TERMINATION_ACTIVE);
    if (previous & TERMINATION_ENABLED) return 1;
    if (previous != TERMINATION_ACTIVE) return 0;
    if (SetConsoleCtrlHandler(termination_console_handler, TRUE)) return 1;
    InterlockedExchange(&termination_state, TERMINATION_ACTIVE);
    return 0;
}

int32_t __oxid_termination_requested(void) {
    return (termination_load() & TERMINATION_REQUESTED) != 0;
}

int32_t __oxid_wait_for_termination(int32_t timeout_ms) {
    if (timeout_ms < 0 || !(termination_load() & TERMINATION_ENABLED)) return 0;
    const ULONGLONG started = GetTickCount64();
    while (!__oxid_termination_requested()) {
        const ULONGLONG elapsed = GetTickCount64() - started;
        if (elapsed >= (ULONGLONG)timeout_ms) break;
        const DWORD remaining = (DWORD)((ULONGLONG)timeout_ms - elapsed);
        Sleep(remaining < 10 ? remaining : 10);
    }
    return __oxid_termination_requested();
}

int32_t __oxid_wait_for_stdin(int32_t timeout_ms) { (void)timeout_ms; return 0; }

void __oxid_termination_scope_end(void) {
    const LONG previous = InterlockedExchange(&termination_state, TERMINATION_ACTIVE);
    if ((previous & TERMINATION_ENABLED) && !SetConsoleCtrlHandler(termination_console_handler, FALSE)) {
        ExitProcess(74);
    }
    InterlockedExchange(&termination_state, 0);
}
#else
int32_t __oxid_termination_scope_start(void) { return -1; }
int32_t __oxid_enable_termination_notifications(void) { return 0; }
int32_t __oxid_termination_requested(void) { return 0; }
int32_t __oxid_wait_for_termination(int32_t timeout_ms) { (void)timeout_ms; return 0; }
int32_t __oxid_wait_for_stdin(int32_t timeout_ms) { (void)timeout_ms; return 0; }
void __oxid_termination_scope_end(void) {}
#endif
#endif

#ifdef OXID_MONOTONIC_MILLIS
#if (defined(__linux__) || defined(__APPLE__)) && \
    (defined(__x86_64__) || defined(__aarch64__)) && UINTPTR_MAX == UINT64_MAX && \
    defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#include <time.h>

_Static_assert(sizeof(struct timespec) == 2 * sizeof(int64_t), "monotonic clock timespec layout");

int64_t __oxid_monotonic_millis(void) {
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec < 0 ||
        now.tv_nsec < 0 || now.tv_nsec >= 1000000000) return -1;
    const int64_t millis = now.tv_nsec / 1000000;
    if (now.tv_sec > (INT64_MAX - millis) / 1000) return -1;
    return (int64_t)now.tv_sec * 1000 + millis;
}
#elif defined(_WIN64) && (defined(_M_X64) || defined(__x86_64__))
#include <windows.h>

int64_t __oxid_monotonic_millis(void) {
    const ULONGLONG millis = GetTickCount64();
    return millis <= (uint64_t)INT64_MAX ? (int64_t)millis : -1;
}
#else
int64_t __oxid_monotonic_millis(void) { return -1; }
#endif
#endif

#ifdef OXID_UNIX_MILLIS
#if (defined(__linux__) || defined(__APPLE__)) && \
    (defined(__x86_64__) || defined(__aarch64__)) && UINTPTR_MAX == UINT64_MAX && \
    defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#include <time.h>

_Static_assert(sizeof(struct timespec) == 2 * sizeof(int64_t), "realtime clock timespec layout");

int64_t __oxid_unix_millis(void) {
    struct timespec now;
    if (clock_gettime(CLOCK_REALTIME, &now) != 0 ||
        now.tv_nsec < 0 || now.tv_nsec >= 1000000000) return INT64_MIN;
    const __int128 millis = (__int128)now.tv_sec * 1000 + now.tv_nsec / 1000000;
    if (millis < INT64_MIN || millis > INT64_MAX) return INT64_MIN;
    return (int64_t)millis;
}
#elif defined(_WIN64) && (defined(_M_X64) || defined(__x86_64__))
#include <windows.h>

int64_t __oxid_unix_millis(void) {
    FILETIME time;
    GetSystemTimeAsFileTime(&time);
    const uint64_t ticks = ((uint64_t)time.dwHighDateTime << 32) | time.dwLowDateTime;
    return (int64_t)(ticks / 10000) - INT64_C(11644473600000);
}
#else
int64_t __oxid_unix_millis(void) { return INT64_MIN; }
#endif
#endif

#ifdef OXID_SLEEP_MILLIS
#if (defined(__linux__) || defined(__APPLE__)) && \
    (defined(__x86_64__) || defined(__aarch64__)) && UINTPTR_MAX == UINT64_MAX && \
    defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#include <errno.h>
#include <time.h>

_Static_assert(sizeof(struct timespec) == 2 * sizeof(int64_t), "sleep timespec layout");

static int sleep_positive(uint32_t milliseconds) {
    struct timespec remaining;
    struct timespec request = {milliseconds / 1000, (long)(milliseconds % 1000) * 1000000};
    while (nanosleep(&request, &remaining) != 0) {
        if (errno != EINTR) return 0;
        request = remaining;
    }
    return 1;
}
#elif defined(_WIN64) && (defined(_M_X64) || defined(__x86_64__))
#include <windows.h>

static int sleep_positive(uint32_t milliseconds) {
    Sleep(milliseconds);
    return 1;
}
#else
static int sleep_positive(uint32_t milliseconds) {
    (void)milliseconds;
    return 0;
}
#endif

int32_t __oxid_sleep_millis(int32_t milliseconds) {
    if (milliseconds <= 0) return milliseconds == 0;
    return sleep_positive((uint32_t)milliseconds);
}
#endif
