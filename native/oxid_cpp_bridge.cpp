#include "oxid_ffi.h"
#include "oxid_cpp_bridge.h"

extern "C" uint64_t oxid_cpp_len(const char* text) {
    return (uint64_t)oxid_c_strlen(text);
}

extern "C" uint64_t oxid_cpp_hash(const char* text) {
    return oxid_c_hash(text);
}
