use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::len as bytes_len;
use alloc::bytes::try_extend;
use alloc::bytes::truncate as truncate_bytes;

pub struct Text {
    bytes: ByteBuf
}

pub enum Validation {
    Valid,
    Invalid(i32)
}

pub enum FromBytes {
    Ok(Text),
    Invalid(ByteBuf)
}

pub enum AppendStatus {
    Ok,
    InvalidInput(i32),
    CapacityOverflow,
    AllocationFailed,
    MemoryLimit
}

pub enum ScalarAppendStatus {
    Ok,
    InvalidScalar(i32),
    CapacityOverflow,
    AllocationFailed,
    MemoryLimit
}

fn continuation(value: i32) -> bool {
    return value >= 128 && value <= 191;
}

fn scalar_width(input: &[u8], offset: i32) -> i32 {
    let lead_byte = input[offset];
    let lead = lead_byte.to_i32();
    let remaining = input.len() - offset;
    if lead <= 127 {
        return 1;
    }
    if lead < 194 {
        return 0;
    }
    if lead <= 223 {
        if remaining < 2 {
            return 0;
        }
        let second_two_byte = input[offset + 1];
        let second_two = second_two_byte.to_i32();
        if !continuation(second_two) {
            return 0;
        }
        return 2;
    }
    if lead <= 239 {
        if remaining < 3 {
            return 0;
        }
        let second_three_byte = input[offset + 1];
        let second_three = second_three_byte.to_i32();
        let third_three_byte = input[offset + 2];
        let third_three = third_three_byte.to_i32();
        if !continuation(second_three) || !continuation(third_three) {
            return 0;
        }
        if lead == 224 && second_three < 160 {
            return 0;
        }
        if lead == 237 && second_three > 159 {
            return 0;
        }
        return 3;
    }
    if lead <= 244 {
        if remaining < 4 {
            return 0;
        }
        let second_four_byte = input[offset + 1];
        let second_four = second_four_byte.to_i32();
        let third_four_byte = input[offset + 2];
        let third_four = third_four_byte.to_i32();
        let fourth_four_byte = input[offset + 3];
        let fourth_four = fourth_four_byte.to_i32();
        if !continuation(second_four) || !continuation(third_four) || !continuation(fourth_four) {
            return 0;
        }
        if lead == 240 && second_four < 144 {
            return 0;
        }
        if lead == 244 && second_four > 143 {
            return 0;
        }
        return 4;
    }
    return 0;
}

pub fn validate(input: &[u8]) -> Validation {
    let mut offset = 0;
    let length = input.len();
    while offset < length {
        let width = scalar_width(&*input, offset);
        if width == 0 {
            return Validation::Invalid(offset);
        }
        offset = offset + width;
    }
    return Validation::Valid;
}

pub fn new_text() -> Text {
    return Text { bytes: new_bytes() };
}

pub fn from_bytes(input: ByteBuf) -> FromBytes {
    let checked = validate(&input);
    match checked {
        Validation::Valid => {
            return FromBytes::Ok(Text { bytes: input });
        },
        Validation::Invalid(invalid_offset) => {
            return FromBytes::Invalid(input);
        },
    }
}

pub fn byte_len(text: &Text) -> i32 {
    return bytes_len(&*text.bytes);
}

fn count_valid_scalars(input: &[u8]) -> i32 {
    let mut count = 0;
    let mut offset = 0;
    while offset < input.len() {
        let current_byte = input[offset];
        let current = current_byte.to_i32();
        if !continuation(current) {
            count = count + 1;
        }
        offset = offset + 1;
    }
    return count;
}

pub fn scalar_count(text: &Text) -> i32 {
    return count_valid_scalars(&*text.bytes);
}

pub fn append_utf8(text: &mut Text, input: &[u8]) -> AppendStatus {
    let checked = validate(&*input);
    match checked {
        Validation::Valid => {},
        Validation::Invalid(invalid_offset) => {
            return AppendStatus::InvalidInput(invalid_offset);
        },
    }
    let grown = try_extend(&mut *text.bytes, &*input);
    match grown {
        GrowStatus::Ok => { return AppendStatus::Ok; },
        GrowStatus::CapacityOverflow => { return AppendStatus::CapacityOverflow; },
        GrowStatus::AllocationFailed => { return AppendStatus::AllocationFailed; },
        GrowStatus::MemoryLimit => { return AppendStatus::MemoryLimit; },
    }
}

fn equal_initialized_bytes(actual: &[u8], input: &[u8]) -> bool {
    if actual.len() != input.len() {
        return false;
    }
    let mut offset = 0;
    while offset < input.len() {
        if actual[offset] != input[offset] {
            return false;
        }
        offset = offset + 1;
    }
    return true;
}

pub fn equals_utf8(text: &Text, input: &[u8]) -> bool {
    return equal_initialized_bytes(&*text.bytes, &*input);
}

pub fn append_scalar(text: &mut Text, scalar: i32) -> ScalarAppendStatus {
    if scalar < 0 || scalar > 1114111 {
        return ScalarAppendStatus::InvalidScalar(scalar);
    }
    if scalar >= 55296 && scalar <= 57343 {
        return ScalarAppendStatus::InvalidScalar(scalar);
    }
    let zero = 0;
    let mut encoded = [zero.to_u8_checked(), zero.to_u8_checked(), zero.to_u8_checked(), zero.to_u8_checked()];
    let mut width = 1;
    if scalar <= 127 {
        encoded[0] = scalar.to_u8_checked();
    }
    if scalar >= 128 && scalar <= 2047 {
        let lead_two = 192 + scalar / 64;
        let tail_two = 128 + scalar % 64;
        encoded[0] = lead_two.to_u8_checked();
        encoded[1] = tail_two.to_u8_checked();
        width = 2;
    }
    if scalar >= 2048 && scalar <= 65535 {
        let lead_three = 224 + scalar / 4096;
        let middle_three = 128 + (scalar / 64) % 64;
        let tail_three = 128 + scalar % 64;
        encoded[0] = lead_three.to_u8_checked();
        encoded[1] = middle_three.to_u8_checked();
        encoded[2] = tail_three.to_u8_checked();
        width = 3;
    }
    if scalar >= 65536 {
        let lead_four = 240 + scalar / 262144;
        let second_four = 128 + (scalar / 4096) % 64;
        let third_four = 128 + (scalar / 64) % 64;
        let tail_four = 128 + scalar % 64;
        encoded[0] = lead_four.to_u8_checked();
        encoded[1] = second_four.to_u8_checked();
        encoded[2] = third_four.to_u8_checked();
        encoded[3] = tail_four.to_u8_checked();
        width = 4;
    }
    let grown = try_extend(&mut *text.bytes, &encoded[0..width]);
    match grown {
        GrowStatus::Ok => { return ScalarAppendStatus::Ok; },
        GrowStatus::CapacityOverflow => { return ScalarAppendStatus::CapacityOverflow; },
        GrowStatus::AllocationFailed => { return ScalarAppendStatus::AllocationFailed; },
        GrowStatus::MemoryLimit => { return ScalarAppendStatus::MemoryLimit; },
    }
}

fn valid_boundary(input: &[u8], length: i32) -> bool {
    let original_length = input.len();
    if length < 0 || length > original_length {
        return false;
    }
    if length == 0 || length == original_length {
        return true;
    }
    let next_byte = input[length];
    let next = next_byte.to_i32();
    return !continuation(next);
}

pub fn truncate(text: &mut Text, length: i32) -> bool {
    if !valid_boundary(&*text.bytes, length) {
        return false;
    }
    truncate_bytes(&mut *text.bytes, length);
    return true;
}
