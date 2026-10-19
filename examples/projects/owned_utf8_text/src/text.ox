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
