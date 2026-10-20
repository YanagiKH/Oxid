pub enum ParseResult {
    Value(i32),
    Empty,
    SignOnly,
    InvalidDigit(i32),
    Overflow(i32)
}

pub enum FormatResult {
    Written(i32),
    BufferTooSmall(i32)
}

// Parse a complete ASCII decimal field. Error payloads are byte offsets.
pub fn parse_i32(input: &[u8]) -> ParseResult {
    let length = input.len();
    if length == 0 {
        return ParseResult::Empty;
    }

    let first_byte = input[0];
    let first = first_byte.to_i32();
    let mut negative = false;
    let mut offset = 0;
    if first == 45 {
        negative = true;
        offset = 1;
    }
    if first == 43 {
        offset = 1;
    }
    if offset == length {
        return ParseResult::SignOnly;
    }

    // Stay nonpositive: MIN is representable, but its positive magnitude is not.
    let mut limit = -2147483647;
    if negative {
        limit = -2147483648;
    }
    let cutoff = limit / 10;
    let mut accumulator = 0;
    while offset < length {
        let current_byte = input[offset];
        let current = current_byte.to_i32();
        if current < 48 || current > 57 {
            return ParseResult::InvalidDigit(offset);
        }
        let digit = current - 48;
        if accumulator < cutoff {
            return ParseResult::Overflow(offset);
        }
        let multiplied = accumulator * 10;
        if multiplied < limit + digit {
            return ParseResult::Overflow(offset);
        }
        accumulator = multiplied - digit;
        offset = offset + 1;
    }

    if negative {
        return ParseResult::Value(accumulator);
    }
    // The nonnegative-input limit excludes MIN, so this negation is safe.
    return ParseResult::Value(-accumulator);
}

// Write canonical ASCII into the caller's prefix, without a terminator.
pub fn format_i32(value: i32, output: &mut [u8]) -> FormatResult {
    let negative = value < 0;
    let mut remaining = value;
    if remaining > 0 {
        remaining = -remaining;
    }

    // Compute the complete width before making any observable output change.
    let mut probe = remaining;
    let mut digits = 1;
    while probe <= -10 {
        probe = probe / 10;
        digits = digits + 1;
    }
    let mut sign_width = 0;
    if negative {
        sign_width = 1;
    }
    let required = digits + sign_width;
    if output.len() < required {
        return FormatResult::BufferTooSmall(required);
    }

    let mut cursor = required;
    while cursor > sign_width {
        cursor = cursor - 1;
        // A nonpositive remainder is in -9..0; only that remainder is negated.
        let digit = -(remaining % 10);
        let ascii = 48 + digit;
        output[cursor] = ascii.to_u8_checked();
        remaining = remaining / 10;
    }
    if negative {
        let minus = 45;
        output[0] = minus.to_u8_checked();
    }
    return FormatResult::Written(required);
}
