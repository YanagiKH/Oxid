#define _GNU_SOURCE
#include "oxid_library.h"
#include <dirent.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>

typedef int32_t (*call_fn)(int32_t, int32_t, int32_t, int32_t *);
typedef uint32_t (*version_fn)(void);
_Static_assert(sizeof(call_fn) == sizeof(void *), "Linux function pointer ABI");
_Static_assert(sizeof(version_fn) == sizeof(void *), "Linux function pointer ABI");
_Static_assert(sizeof(int32_t) == 4 && sizeof(uint32_t) == 4, "fixed-width ABI");

static void pipe_handler(int number) { (void)number; }
static int fail(const char *message) {
    fprintf(stderr, "native C client: %s\n", message);
    return 1;
}
static int descriptor_count(void) {
    DIR *directory = opendir("/proc/self/fd");
    if (!directory) return -1;
    int count = 0;
    struct dirent *entry;
    while ((entry = readdir(directory))) {
        if (strcmp(entry->d_name, ".") && strcmp(entry->d_name, "..")) ++count;
    }
    return closedir(directory) ? -1 : count;
}

static int examples(call_fn call, int fuel_only) {
    static const struct {
        int32_t unit, quantity, divisor, status, value;
    } cases[] = {
        {1250, 3, 2, 0, 1875}, {-7, 1, 2, 0, -3}, {0, 1, 1, 0, 0},
        {INT32_MIN, 1, 1, 0, INT32_MIN}, {INT32_MAX, 1, 1, 0, INT32_MAX},
        {INT32_MAX, 2, 1, 1, 0}, {INT32_MIN, 1, -1, 1, 0}, {1, 1, 0, 2, 0},
        {42, 1, 1, 0, 42},
    };
    for (size_t i = 0; i < sizeof cases / sizeof cases[0]; ++i) {
        int32_t output = INT32_C(123456789);
        int32_t status = call(cases[i].unit, cases[i].quantity, cases[i].divisor, &output);
        int32_t expected_status = fuel_only ? 3 : cases[i].status;
        int32_t expected_value = expected_status ? INT32_C(123456789) : cases[i].value;
        if (status != expected_status || output != expected_value) return 0;
    }
    if (call(1, 1, 1, NULL) != 4) return 0;
    /* Manufacture only an argument value; the client never dereferences it. */
    _Alignas(int32_t) unsigned char bytes[3 * sizeof(int32_t)];
    unsigned char before[sizeof bytes];
    memset(bytes, 0xa5, sizeof bytes);
    memcpy(before, bytes, sizeof bytes);
    uintptr_t address = (uintptr_t)bytes + 1;
    int32_t *misaligned;
    _Static_assert(sizeof address == sizeof misaligned, "Linux data pointer ABI");
    memcpy(&misaligned, &address, sizeof misaligned);
    if (call(1, 1, 1, misaligned) != 4 || memcmp(bytes, before, sizeof bytes)) return 0;
    return 1;
}

typedef struct {
    call_fn call;
    int fuel_only;
    int passed;
    pthread_barrier_t *barrier;
} worker_input;
static void *worker(void *opaque) {
    worker_input *input = opaque;
    int barrier = pthread_barrier_wait(input->barrier);
    if (barrier && barrier != PTHREAD_BARRIER_SERIAL_THREAD) return NULL;
    input->passed = 1;
    for (unsigned i = 0; i < 128; ++i) {
        if (!examples(input->call, input->fuel_only)) { input->passed = 0; break; }
    }
    return NULL;
}

int main(int argc, char **argv) {
    if (argc != 3 || (strcmp(argv[2], "pricing") && strcmp(argv[2], "fuel")))
        return fail("expected copied-library path and pricing|fuel mode");
    int fuel_only = !strcmp(argv[2], "fuel");
    struct sigaction saved, installed, after;
    memset(&installed, 0, sizeof installed);
    installed.sa_handler = pipe_handler;
    if (sigemptyset(&installed.sa_mask) || sigaction(SIGPIPE, &installed, &saved)
        || sigaction(SIGPIPE, NULL, &installed)) return fail("SIGPIPE setup failed");
    int descriptor_flags[3], status_flags[3];
    struct stat descriptors[3];
    for (int fd = 0; fd < 3; ++fd) {
        descriptor_flags[fd] = fcntl(fd, F_GETFD);
        status_flags[fd] = fcntl(fd, F_GETFL);
        if (descriptor_flags[fd] < 0 || status_flags[fd] < 0 || fstat(fd, &descriptors[fd]))
            return fail("standard descriptor snapshot failed");
    }
    int before_count = descriptor_count();
    if (before_count < 0 || oxid_library_abi_version() != 1) return fail("ABI/snapshot failed");
    for (unsigned cycle = 0; cycle < 3; ++cycle) {
        void *handle = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
        if (!handle) return fail("independent copied-library load failed");
        void *symbol = dlsym(handle, "oxid_library_call_v1");
        call_fn independent;
        memcpy(&independent, &symbol, sizeof independent);
        symbol = dlsym(handle, "oxid_library_abi_version");
        version_fn version;
        memcpy(&version, &symbol, sizeof version);
        if (!independent || !version || version() != 1
            || !examples(oxid_library_call_v1, fuel_only) || !examples(independent, fuel_only))
            return fail("direct/nested error or publication contract failed");
        pthread_barrier_t barrier;
        if (pthread_barrier_init(&barrier, NULL, 4)) return fail("barrier init failed");
        pthread_t threads[4];
        worker_input inputs[4];
        for (unsigned i = 0; i < 4; ++i) {
            inputs[i] = (worker_input){i & 1 ? independent : oxid_library_call_v1,
                fuel_only, 0, &barrier};
            if (pthread_create(&threads[i], NULL, worker, &inputs[i])) return fail("thread create failed");
        }
        for (unsigned i = 0; i < 4; ++i) {
            if (pthread_join(threads[i], NULL) || !inputs[i].passed) return fail("concurrent call failed");
        }
        if (pthread_barrier_destroy(&barrier) || dlclose(handle)) return fail("unload failed");
    }
    if (sigaction(SIGPIPE, NULL, &after) || after.sa_handler != installed.sa_handler
        || after.sa_flags != installed.sa_flags) return fail("library changed SIGPIPE action");
    for (int number = 1; number < NSIG; ++number) {
        if (sigismember(&after.sa_mask, number) != sigismember(&installed.sa_mask, number))
            return fail("library changed SIGPIPE mask");
    }
    for (int fd = 0; fd < 3; ++fd) {
        struct stat current;
        if (fstat(fd, &current) || fcntl(fd, F_GETFD) != descriptor_flags[fd]
            || fcntl(fd, F_GETFL) != status_flags[fd] || current.st_dev != descriptors[fd].st_dev
            || current.st_ino != descriptors[fd].st_ino || current.st_mode != descriptors[fd].st_mode)
            return fail("library changed a standard descriptor");
    }
    if (descriptor_count() != before_count) return fail("library retained a file descriptor");
    if (sigaction(SIGPIPE, &saved, NULL)) return fail("SIGPIPE restore failed");
    puts("native C calls passed");
    return 0;
}
