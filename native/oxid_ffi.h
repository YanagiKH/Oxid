#ifndef OXID_FFI_H
#define OXID_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

size_t oxid_c_strlen(const char *s);
uint64_t oxid_c_hash(const char *s);

#ifdef __cplusplus
}
#endif

#endif
