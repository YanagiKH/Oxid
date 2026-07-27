#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

uint64_t oxid_cpp_len(const char* text);
uint64_t oxid_cpp_hash(const char* text);

#ifdef __cplusplus
}
#endif
