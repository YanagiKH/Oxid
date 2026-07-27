#ifndef OXID_FFI_H
#define OXID_FFI_H

#include <stddef.h>
#include <stdint.h>

size_t oxid_c_strlen(const char *s);
uint64_t oxid_c_hash(const char *s);

#endif
