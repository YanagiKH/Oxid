/* Included only for the explicitly admitted process-notification family.
 * _POSIX_C_SOURCE must be selected before the first runtime system header. */
#include <stdint.h>

#if !defined(OXID_TERMINATION_FORCE_UNSUPPORTED) && \
    (defined(__linux__) || defined(__APPLE__)) && \
    (defined(__x86_64__) || defined(__aarch64__)) && UINTPTR_MAX == UINT64_MAX && \
    defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#include <signal.h>
#include <stddef.h>
#include <unistd.h>

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
#else
int32_t __oxid_termination_scope_start(void) { return -1; }
int32_t __oxid_enable_termination_notifications(void) { return 0; }
int32_t __oxid_termination_requested(void) { return 0; }
void __oxid_termination_scope_end(void) {}
#endif
