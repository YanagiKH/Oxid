pub enum EncodeResult {
    Written(i32),
    BufferTooSmall(i32),
    LengthOverflow
}

pub enum DecodeResult {
    Written(i32),
    OddLength,
    InvalidDigit(i32),
    BufferTooSmall(i32)
}

// Only callers with a nibble in 0..15 use this helper.
fn encode_digit(value: i32) -> u8 {
    if value < 10 {
        let digit = 48 + value;
        return digit.to_u8_checked();
    }
    let letter = 87 + value;
    return letter.to_u8_checked();
}

// Return a nibble in 0..15, or -1 for any non-hexadecimal byte.
fn decode_digit(value: u8) -> i32 {
    let ascii = value.to_i32();
    if ascii >= 48 && ascii <= 57 {
        return ascii - 48;
    }
    if ascii >= 65 && ascii <= 70 {
        return ascii - 55;
    }
    if ascii >= 97 && ascii <= 102 {
        return ascii - 87;
    }
    return -1;
}

// Write lowercase ASCII into the caller's prefix, without a terminator.
pub fn encode_lower(input: &[u8], output: &mut [u8]) -> EncodeResult {
    let length = input.len();
    if length > 1073741823 {
        return EncodeResult::LengthOverflow;
    }
    let required = length * 2;
    if output.len() < required {
        return EncodeResult::BufferTooSmall(required);
    }

    let mut offset = 0;
    let mut cursor = 0;
    while offset < length {
        let current_byte = input[offset];
        let current = current_byte.to_i32();
        output[cursor] = encode_digit(current / 16);
        output[cursor + 1] = encode_digit(current % 16);
        offset = offset + 1;
        cursor = cursor + 2;
    }
    return EncodeResult::Written(required);
}

// Validate the complete input before checking capacity or writing any byte.
pub fn decode(input: &[u8], output: &mut [u8]) -> DecodeResult {
    let length = input.len();
    if length % 2 != 0 {
        return DecodeResult::OddLength;
    }

    let mut checked = 0;
    while checked < length {
        let current_byte = input[checked];
        if decode_digit(current_byte) < 0 {
            return DecodeResult::InvalidDigit(checked);
        }
        checked = checked + 1;
    }

    let required = length / 2;
    if output.len() < required {
        return DecodeResult::BufferTooSmall(required);
    }

    let mut offset = 0;
    let mut cursor = 0;
    while offset < length {
        let high_byte = input[offset];
        let low_byte = input[offset + 1];
        let high = decode_digit(high_byte);
        let low = decode_digit(low_byte);
        let decoded = high * 16 + low;
        output[cursor] = decoded.to_u8_checked();
        offset = offset + 2;
        cursor = cursor + 1;
    }
    return DecodeResult::Written(required);
}
