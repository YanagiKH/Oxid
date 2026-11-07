/* Batch transport only; expected arithmetic and decimal output live in Python. */
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include "../native/typed_preview.c"
#include "../native/typed_integer128.c"

int main(void) {
    if (setvbuf(stdout, NULL, _IONBF, 0) != 0) return 2;
    char line[160];
    while (fgets(line, sizeof line, stdin) != NULL) {
        char operation;
        uint64_t a_low = 0, a_high = 0, b_low = 0, b_high = 0;
        const int count = sscanf(line, "%c %" SCNx64 " %" SCNx64 " %" SCNx64 " %" SCNx64,
                                 &operation, &a_low, &a_high, &b_low, &b_high);
        if (operation == 'u' || operation == 'i') {
            if (count != 3) return 2;
            const int status = operation == 'u' ? __oxid_print_u128_parts(a_low, a_high)
                                                : __oxid_print_i128_parts(a_low, a_high);
            if (status != 0) return status;
        } else if (operation == 'm' || operation == 's') {
            if (count != 5) return 2;
            uint64_t low = UINT64_C(0x123456789abcdef0), high = UINT64_C(0xfedcba9876543210);
            const int32_t status = __oxid_checked_mul128_parts(a_low, a_high, b_low, b_high,
                                                              operation == 's', &low, &high);
            printf("%" PRId32 " %016" PRIx64 " %016" PRIx64 "\n", status, low, high);
        } else if (operation == 'd') {
            if (count != 5) return 2;
            uint64_t q_low = 1, q_high = 2, r_low = 3, r_high = 4;
            const int32_t status = __oxid_udivmod128_parts(a_low, a_high, b_low, b_high,
                                                          &q_low, &q_high, &r_low, &r_high);
            printf("%" PRId32 " %016" PRIx64 " %016" PRIx64 " %016" PRIx64 " %016" PRIx64 "\n",
                   status, q_low, q_high, r_low, r_high);
        } else {
            return 2;
        }
    }
    return ferror(stdin) ? 2 : 0;
}
