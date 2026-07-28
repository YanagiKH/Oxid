#include "oxid_ffi.h"
#include <string.h>

size_t oxid_c_strlen(const char *s) {
    return s ? strlen(s) : 0;
}

uint64_t oxid_c_hash(const char *s) {
    if (!s) return 0;
    uint64_t hash = 1469598103934665603ULL;
    while (*s) {
        hash ^= (unsigned char)(*s++);
        hash *= 1099511628211ULL;
    }
    return hash;
}
