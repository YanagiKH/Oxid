/* Immutable process arguments. The entry adapter installs OS-owned strings;
 * no pointer escapes into the language and no argument is ever modified. */
#if defined(_WIN32) || defined(OXID_ARGUMENTS_UTF16)
typedef uint16_t oxid_argument_unit;

static uint32_t argument_character(const oxid_argument_unit **source) {
    uint32_t unit = *(*source)++;
    if (unit >= 0xd800 && unit <= 0xdbff) {
        uint32_t next = **source;
        if (next >= 0xdc00 && next <= 0xdfff) {
            ++*source;
            return 0x10000 + ((unit - 0xd800) << 10) + (next - 0xdc00);
        }
    }
    return unit >= 0xd800 && unit <= 0xdfff ? 0xfffd : unit;
}

static uint32_t argument_width(uint32_t character) {
    if (character < 0x80) return 1;
    if (character < 0x800) return 2;
    return character < 0x10000 ? 3 : 4;
}

static void argument_copy(uint8_t *bytes, const oxid_argument_unit *source, size_t length) {
    const uint8_t leads[] = {0, 0, 0xc0, 0xe0, 0xf0};
    uint8_t *end = bytes + length;
    while (bytes < end) {
        uint32_t character = argument_character(&source);
        uint32_t width = argument_width(character);
        for (uint32_t position = width - 1; position > 0; --position) {
            bytes[position] = 0x80 | (character & 0x3f);
            character >>= 6;
        }
        bytes[0] = leads[width] | character;
        bytes += width;
    }
}
#else
typedef char oxid_argument_unit;

static void argument_copy(uint8_t *bytes, const oxid_argument_unit *source, size_t length) {
    memcpy(bytes, source, length);
}
#endif

typedef struct { int32_t count; oxid_argument_unit **values; } oxid_arguments_context;
static oxid_arguments_context program_arguments;
_Static_assert(sizeof(oxid_arguments_context) == 16, "argument context bytes");

void __oxid_arguments_start(int32_t count, oxid_argument_unit **values) {
    program_arguments = count > 0 && values
        ? (oxid_arguments_context){count, values}
        : (oxid_arguments_context){0, NULL};
}
int32_t __oxid_arg_count(void) { return program_arguments.count; }

static int32_t argument_descriptor(uint8_t *bytes, int32_t capacity,
    uint64_t *fuel, oxid_binary_result *result) {
    if (capacity < 0) return OXID_BUFFER_BOUNDS;
    if (!fuel || !result || (capacity && !bytes)) return OXID_BUFFER_DESCRIPTOR;
    if (overlaps(bytes, (size_t)capacity, result, sizeof(*result)) ||
        overlaps(bytes, (size_t)capacity, fuel, sizeof(*fuel)) ||
        overlaps(result, sizeof(*result), fuel, sizeof(*fuel))) return OXID_BUFFER_DESCRIPTOR;
    return 0;
}

/* Scan with a local budget, then prepay the copy and canonical return. Even a
 * fuel refusal after a partial scan leaves the destination and result intact. */
int32_t __oxid_arg_bytes(int32_t index, uint8_t *bytes, int32_t capacity,
    uint64_t *fuel, oxid_binary_result *result, uint64_t return_cost) {
    int32_t admission = argument_descriptor(bytes, capacity, fuel, result);
    if (admission) return admission;
    uint64_t remaining = *fuel;
    oxid_binary_result value = {2, 0};
    const oxid_argument_unit *source = NULL;
    uint64_t length = 0;
    if (index >= 0 && index < program_arguments.count) {
        source = program_arguments.values[index];
        if (!source) return OXID_BUFFER_DESCRIPTOR;
#if defined(_WIN32) || defined(OXID_ARGUMENTS_UTF16)
        const oxid_argument_unit *cursor = source;
        for (;;) {
            if (!remaining) return OXID_BUFFER_FUEL;
            uint32_t character = argument_character(&cursor);
            uint64_t width = argument_width(character);
            if (remaining < width) return OXID_BUFFER_FUEL;
            remaining -= width;
            if (!character) break;
            length += width;
            if (length > INT32_MAX) break;
        }
#else
        for (;;) {
            if (!remaining) return OXID_BUFFER_FUEL;
            --remaining;
            if (!source[length]) break;
            ++length;
            if (length > INT32_MAX) break;
        }
#endif
        if (length > INT32_MAX) value.tag = 3;
        else {
            value = (oxid_binary_result){length > (uint64_t)capacity ? 1 : 0, (int32_t)length};
            if (!value.tag) {
                if (remaining < length) return OXID_BUFFER_FUEL;
                remaining -= length;
            }
        }
    }
    if (remaining < return_cost) return OXID_BUFFER_FUEL;
    *fuel = remaining - return_cost;
    if (!value.tag && length) argument_copy(bytes, source, (size_t)length);
    *result = value;
    return 0;
}
int32_t __oxid_arg_bytes_hosted(int32_t index, uint8_t *bytes, int32_t capacity,
    uint64_t *fuel, oxid_binary_result *result, uint64_t return_cost) {
    /* Validate the original pointer before replacing only its work budget. */
    int32_t admission = argument_descriptor(bytes, capacity, fuel, result);
    if (admission) return admission;
    uint64_t operation_budget = UINT64_MAX;
    return __oxid_arg_bytes(index, bytes, capacity, &operation_budget, result, return_cost);
}

#if defined(OXID_READ_ENVIRONMENT)
#if defined(_WIN32)
#include <wchar.h>
#endif
enum { ENV_NAME_LIMIT = 255, ENV_VALUE_LIMIT = 32768 };
_Static_assert(EINVAL == 22 && ERANGE == 34, "environment error codes must match the interpreter");

static int32_t environment_descriptor(const uint8_t *name, int32_t name_length,
    uint8_t *bytes, int32_t capacity, uint64_t *fuel, oxid_binary_result *result) {
    if (name_length < 0) return OXID_BUFFER_BOUNDS;
    if (name_length && !name) return OXID_BUFFER_DESCRIPTOR;
    int32_t status = argument_descriptor(bytes, capacity, fuel, result);
    if (status) return status;
    size_t length = (size_t)name_length;
    if (overlaps(name, length, bytes, (size_t)capacity) ||
        overlaps(name, length, result, sizeof(*result)) ||
        overlaps(name, length, fuel, sizeof(*fuel))) return OXID_BUFFER_DESCRIPTOR;
    return 0;
}

int32_t __oxid_environment_bytes(const uint8_t *name, int32_t name_length,
    uint8_t *bytes, int32_t capacity, uint64_t *fuel, oxid_binary_result *result,
    uint64_t return_cost) {
    int32_t status = environment_descriptor(name, name_length, bytes, capacity, fuel, result);
    if (status) return status;
    if (*fuel < return_cost) return OXID_BUFFER_FUEL;
    uint64_t remaining = *fuel - return_cost;
    oxid_binary_result value = {2, EINVAL};
    const oxid_argument_unit *source = NULL;
    uint32_t length = 0;
    if (name_length > 0 && name_length <= ENV_NAME_LIMIT) {
        if (remaining < (uint64_t)name_length + 1) return OXID_BUFFER_FUEL;
        remaining -= (uint64_t)name_length + 1;
        oxid_argument_unit terminated[ENV_NAME_LIMIT + 1];
        int valid = 1;
        for (int32_t index = 0; index < name_length; ++index) {
            uint8_t character = name[index];
            if (!character || character == '=' || character > 127) {
                valid = 0;
                break;
            }
            terminated[index] = (oxid_argument_unit)character;
        }
        if (valid) {
            terminated[name_length] = 0;
#if defined(_WIN32)
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
            source = (const oxid_argument_unit *)_wgetenv((const wchar_t *)terminated);
#pragma clang diagnostic pop
#else
            source = getenv(terminated);
#endif
            value = (oxid_binary_result){1, 0};
        }
    }
    if (source) {
#if defined(_WIN32) || defined(OXID_ARGUMENTS_UTF16)
        const oxid_argument_unit *cursor = source;
        for (;;) {
            uint32_t character = argument_character(&cursor);
            uint32_t width = argument_width(character);
            if (remaining < width) return OXID_BUFFER_FUEL;
            remaining -= width;
            if (!character) break;
            length += width;
            if (length > ENV_VALUE_LIMIT) break;
        }
#else
        for (;;) {
            if (!remaining) return OXID_BUFFER_FUEL;
            --remaining;
            if (!source[length]) break;
            ++length;
            if (length > ENV_VALUE_LIMIT) break;
        }
#endif
        if (length > ENV_VALUE_LIMIT || length > (uint32_t)capacity) {
            value = (oxid_binary_result){2, ERANGE};
        } else {
            if (remaining < length) return OXID_BUFFER_FUEL;
            remaining -= length;
            value = (oxid_binary_result){0, (int32_t)length};
        }
    }
    *fuel = remaining;
    if (!value.tag && length) argument_copy(bytes, source, length);
    *result = value;
    return 0;
}

int32_t __oxid_environment_bytes_hosted(const uint8_t *name, int32_t name_length,
    uint8_t *bytes, int32_t capacity, uint64_t *fuel, oxid_binary_result *result,
    uint64_t return_cost) {
    int32_t status = environment_descriptor(name, name_length, bytes, capacity, fuel, result);
    if (status) return status;
    uint64_t budget = UINT64_MAX;
    return __oxid_environment_bytes(name, name_length, bytes, capacity, &budget, result, return_cost);
}
#endif
