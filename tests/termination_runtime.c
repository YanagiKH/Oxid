#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#if !defined(OXID_TERMINATION_FORCE_UNSUPPORTED)
static int fail_install;
static int attempted_installs;
static int selected_install(int number, const struct sigaction *action,
                            struct sigaction *previous, int step) {
    ++attempted_installs;
    return fail_install == step ? -1 : sigaction(number, action, previous);
}
#define OXID_TERMINATION_INSTALL(number, action, previous, step) \
    selected_install((number), (action), (previous), (step))
#endif
#include "../native/typed_termination.c"

static volatile sig_atomic_t old_notification;
static void old_handler(int number) { old_notification = number; }

static struct sigaction disposition(int number) {
    struct sigaction result;
    assert(sigaction(number, NULL, &result) == 0);
    return result;
}
static void same_disposition(int number, const struct sigaction *expected) {
    struct sigaction actual = disposition(number);
    assert(actual.sa_handler == expected->sa_handler);
    assert(actual.sa_flags == expected->sa_flags);
    for (int candidate = 1; candidate < 128; ++candidate)
        assert(sigismember(&actual.sa_mask, candidate) ==
               sigismember(&expected->sa_mask, candidate));
}
static void default_signal_without_opt_in(void) {
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        struct sigaction action = {0};
        action.sa_handler = SIG_DFL;
        assert(sigemptyset(&action.sa_mask) == 0);
        assert(sigaction(SIGTERM, &action, NULL) == 0);
        (void)__oxid_termination_scope_start();
        assert(raise(SIGTERM) == 0);
        _exit(93);
    }
    int status = 0;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGTERM);
}

int main(void) {
    struct sigaction old = {0};
    old.sa_handler = old_handler;
    old.sa_flags = SA_RESTART;
    assert(sigemptyset(&old.sa_mask) == 0);
    assert(sigaddset(&old.sa_mask, SIGUSR1) == 0);
    assert(sigaction(SIGINT, &old, NULL) == 0);
    assert(sigaction(SIGTERM, &old, NULL) == 0);
    struct sigaction before_int = disposition(SIGINT);
    struct sigaction before_term = disposition(SIGTERM);
    sigset_t before_mask;
    assert(sigprocmask(SIG_SETMASK, NULL, &before_mask) == 0);
    assert(__oxid_enable_termination_notifications() == 0);
    assert(__oxid_termination_requested() == 0);
    assert(__oxid_wait_for_termination(2000000000) == 0);

#if defined(OXID_TERMINATION_FORCE_UNSUPPORTED)
    assert(__oxid_termination_scope_start() == -1);
    assert(__oxid_enable_termination_notifications() == 0);
    assert(__oxid_termination_requested() == 0);
    __oxid_termination_scope_end();
#else
    assert(__oxid_termination_scope_start() == 0);
    assert(__oxid_termination_scope_start() == -1);
    assert(__oxid_wait_for_termination(2000000000) == 0);
    assert(raise(SIGINT) == 0);
    assert(old_notification == SIGINT);
    assert(__oxid_termination_requested() == 0);
    for (int step = 1; step <= 2; ++step) {
        fail_install = step;
        attempted_installs = 0;
        assert(__oxid_enable_termination_notifications() == 0);
        assert(attempted_installs == step);
        assert(__oxid_termination_requested() == 0);
        same_disposition(SIGINT, &before_int);
        same_disposition(SIGTERM, &before_term);
    }
    fail_install = 0;
    assert(__oxid_enable_termination_notifications() == 1);
    assert(__oxid_termination_requested() == 0);
    assert(__oxid_wait_for_termination(0) == 0);
    struct timespec started, finished;
    assert(clock_gettime(CLOCK_MONOTONIC, &started) == 0);
    assert(__oxid_wait_for_termination(25) == 0);
    assert(clock_gettime(CLOCK_MONOTONIC, &finished) == 0);
    int64_t elapsed_ns = (finished.tv_sec - started.tv_sec) * INT64_C(1000000000)
        + finished.tv_nsec - started.tv_nsec;
    assert(elapsed_ns >= 25 * INT64_C(1000000));
    assert(raise(SIGINT) == 0);
    assert(__oxid_termination_requested() == 1);
    assert(__oxid_wait_for_termination(-1) == 0);
    assert(__oxid_wait_for_termination(0) == 1);
    assert(__oxid_wait_for_termination(2000000000) == 1);
    assert(__oxid_enable_termination_notifications() == 1);
    assert(__oxid_termination_requested() == 1);
    assert(raise(SIGTERM) == 0);
    assert(__oxid_termination_requested() == 1);
    __oxid_termination_scope_end();
    assert(__oxid_termination_requested() == 0);
    assert(__oxid_enable_termination_notifications() == 0);
    assert(raise(SIGTERM) == 0);
    assert(old_notification == SIGTERM);
    assert(__oxid_termination_scope_start() == 0);
    assert(__oxid_enable_termination_notifications() == 1);
    assert(__oxid_termination_requested() == 0);
    __oxid_termination_scope_end();
#endif
    __oxid_termination_scope_end();
    same_disposition(SIGINT, &before_int);
    same_disposition(SIGTERM, &before_term);
    sigset_t after_mask;
    assert(sigprocmask(SIG_SETMASK, NULL, &after_mask) == 0);
    for (int candidate = 1; candidate < 128; ++candidate)
        assert(sigismember(&before_mask, candidate) ==
               sigismember(&after_mask, candidate));
    default_signal_without_opt_in();
    return 0;
}
