/* Private 128-bit arithmetic ABI using explicit 64-bit parts. Include after
 * typed_preview.c. No C 128-bit extension, allocation or compiler builtins. */

static void oxid_negate128(uint64_t *low, uint64_t *high) {
    *low = UINT64_C(0) - *low;
    *high = ~*high + (uint64_t)(*low == 0);
}

/* On overflow, neither result word is changed. */
int32_t __oxid_checked_mul128_parts(uint64_t a_low, uint64_t a_high,
                                   uint64_t b_low, uint64_t b_high,
                                   int32_t signed_kind,
                                   uint64_t *result_low, uint64_t *result_high) {
    const int negative_a = signed_kind != 0 && (a_high >> 63) != 0;
    const int negative_b = signed_kind != 0 && (b_high >> 63) != 0;
    if (negative_a) oxid_negate128(&a_low, &a_high);
    if (negative_b) oxid_negate128(&b_low, &b_high);
    const uint32_t a[4] = {(uint32_t)a_low, (uint32_t)(a_low >> 32),
                           (uint32_t)a_high, (uint32_t)(a_high >> 32)};
    const uint32_t b[4] = {(uint32_t)b_low, (uint32_t)(b_low >> 32),
                           (uint32_t)b_high, (uint32_t)(b_high >> 32)};
    uint32_t product[8] = {0};
    for (size_t i = 0; i < 4; ++i) {
        uint64_t carry = 0;
        for (size_t j = 0; j < 4; ++j) {
            const uint64_t value = (uint64_t)a[i] * b[j] + product[i + j] + carry;
            product[i + j] = (uint32_t)value;
            carry = value >> 32;
        }
        product[i + 4] = (uint32_t)carry;
    }
    if ((product[4] | product[5] | product[6] | product[7]) != 0) return 1;
    uint64_t low = (uint64_t)product[0] | ((uint64_t)product[1] << 32);
    uint64_t high = (uint64_t)product[2] | ((uint64_t)product[3] << 32);
    const int negative = negative_a != negative_b;
    if (signed_kind != 0) {
        if (negative) {
            if (high > UINT64_C(0x8000000000000000) ||
                (high == UINT64_C(0x8000000000000000) && low != 0)) return 1;
            oxid_negate128(&low, &high);
        } else if ((high >> 63) != 0) {
            return 1;
        }
    }
    *result_low = low;
    *result_high = high;
    return 0;
}

/* Fixed 128-step unsigned division. The extra carry retains the 129th
 * remainder bit before subtraction. A zero divisor changes no output word. */
int32_t __oxid_udivmod128_parts(uint64_t dividend_low, uint64_t dividend_high,
                               uint64_t divisor_low, uint64_t divisor_high,
                               uint64_t *quotient_low, uint64_t *quotient_high,
                               uint64_t *remainder_low, uint64_t *remainder_high) {
    if ((divisor_low | divisor_high) == 0) return 1;
    uint64_t q_low = 0, q_high = 0, r_low = 0, r_high = 0;
    for (size_t bit = 0; bit < 128; ++bit) {
        const uint64_t incoming = dividend_high >> 63;
        dividend_high = (dividend_high << 1) | (dividend_low >> 63);
        dividend_low <<= 1;
        const uint64_t carry = r_high >> 63;
        r_high = (r_high << 1) | (r_low >> 63);
        r_low = (r_low << 1) | incoming;
        q_high = (q_high << 1) | (q_low >> 63);
        q_low <<= 1;
        if (carry != 0 || r_high > divisor_high ||
            (r_high == divisor_high && r_low >= divisor_low)) {
            const uint64_t borrow = (uint64_t)(r_low < divisor_low);
            r_low -= divisor_low;
            r_high = r_high - divisor_high - borrow;
            q_low |= 1;
        }
    }
    *quotient_low = q_low;
    *quotient_high = q_high;
    *remainder_low = r_low;
    *remainder_high = r_high;
    return 0;
}

static unsigned oxid_decimal_step128(uint64_t *low, uint64_t *high) {
    const uint32_t input[4] = {(uint32_t)(*high >> 32), (uint32_t)*high,
                               (uint32_t)(*low >> 32), (uint32_t)*low};
    uint32_t quotient[4];
    uint64_t remainder = 0;
    for (size_t i = 0; i < 4; ++i) {
        const uint64_t value = (remainder << 32) | input[i];
        quotient[i] = (uint32_t)(value / 10);
        remainder = value % 10;
    }
    *high = ((uint64_t)quotient[0] << 32) | quotient[1];
    *low = ((uint64_t)quotient[2] << 32) | quotient[3];
    return (unsigned)remainder;
}

static int oxid_print128_parts(uint64_t low, uint64_t high, int negative) {
    char buffer[41]; /* sign, up to 39 digits, newline */
    size_t index = sizeof buffer;
    buffer[--index] = '\n';
    do {
        buffer[--index] = (char)('0' + oxid_decimal_step128(&low, &high));
    } while ((low | high) != 0);
    if (negative) buffer[--index] = '-';
    return output(STDOUT_FILENO, buffer + index, sizeof buffer - index);
}

int __oxid_print_u128_parts(uint64_t low, uint64_t high) {
    return oxid_print128_parts(low, high, 0);
}

int __oxid_print_i128_parts(uint64_t low, uint64_t high) {
    const int negative = (high >> 63) != 0;
    if (negative) oxid_negate128(&low, &high);
    return oxid_print128_parts(low, high, negative);
}
