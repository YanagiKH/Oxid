/* Immutable process arguments. The entry adapter installs OS-owned strings;
 * no pointer escapes into the language and no argument is ever modified. */
typedef struct { int32_t count; char **values; } oxid_arguments_context;
static oxid_arguments_context program_arguments;
_Static_assert(sizeof(oxid_arguments_context) == 16, "argument context bytes");

void __oxid_arguments_start(int32_t count, char **values) {
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
    const uint8_t *source = NULL;
    uint64_t length = 0;
    if (index >= 0 && index < program_arguments.count) {
        source = (const uint8_t *)program_arguments.values[index];
        if (!source) return OXID_BUFFER_DESCRIPTOR;
        for (;;) {
            if (!remaining) return OXID_BUFFER_FUEL;
            --remaining;
            if (!source[length]) break;
            ++length;
            if (length > INT32_MAX) break;
        }
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
    if (!value.tag && length) memcpy(bytes, source, (size_t)length);
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
