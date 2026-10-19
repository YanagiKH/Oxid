use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use text::Validation;
use text::FromBytes;
use text::validate;
use text::from_bytes;
use text::byte_len;
use text::scalar_count;
use text::equals_utf8;

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn equal_bytes(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut index = 0;
    while index < actual.len() {
        if actual[index] != expected[index] {
            return false;
        }
        index = index + 1;
    }
    return true;
}

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn is_valid(input: &[u8]) -> bool {
    let checked = validate(&*input);
    match checked {
        Validation::Valid => { return true; },
        Validation::Invalid(offset) => { return false; },
    }
}

fn invalid_at(input: &[u8], expected_offset: i32) -> bool {
    let checked = validate(&*input);
    match checked {
        Validation::Valid => { return false; },
        Validation::Invalid(actual_offset) => { return actual_offset == expected_offset; },
    }
}

fn check_ascii() -> i32 {
    let ascii = b"Oxid 42!";
    let expected = [byte(79), byte(120), byte(105), byte(100), byte(32), byte(52), byte(50), byte(33)];
    if ascii.len() != 8 || !equal_bytes(&ascii, &expected) { return -1; }
    let edges = b" ~'";
    let expected_edges = [byte(32), byte(126), byte(39)];
    if edges.len() != 3 || !equal_bytes(&edges, &expected_edges) { return -2; }
    return 0;
}

fn check_empty() -> i32 {
    let annotated: [u8; 0] = b"";
    let inferred = b"";
    let control: [u8; 0] = [];
    if annotated.len() != 0 || inferred.len() != 0 { return -3; }
    if !equal_bytes(&annotated, &control) || !equal_bytes(&inferred, &control) { return -4; }
    if !is_valid(&inferred) { return -5; }
    let mut owner = new();
    if !grew(try_extend(&mut owner, &inferred)) || len(&owner) != 0 { return -6; }
    return 0;
}

fn check_nul_and_high_bytes() -> i32 {
    let high = b"\0\x80\xFF\x00";
    let expected = [byte(0), byte(128), byte(255), byte(0)];
    if high.len() != 4 || !equal_bytes(&high, &expected) { return -7; }
    return 0;
}

fn check_short_escapes() -> i32 {
    let escaped = b"\"\\\n\r\t\0";
    let expected = [byte(34), byte(92), byte(10), byte(13), byte(9), byte(0)];
    if escaped.len() != 6 || !equal_bytes(&escaped, &expected) { return -8; }
    let mixed = b"a\\\"b";
    let expected_mixed = [byte(97), byte(92), byte(34), byte(98)];
    if mixed.len() != 4 || !equal_bytes(&mixed, &expected_mixed) { return -9; }
    return 0;
}

fn check_hex_case() -> i32 {
    let hex = b"\x4f\x4F\xaB\xAb\x7e\x7E\x01";
    let expected = [byte(79), byte(79), byte(171), byte(171), byte(126), byte(126), byte(1)];
    if hex.len() != 7 || !equal_bytes(&hex, &expected) { return -10; }
    return 0;
}

fn make() -> [u8; 3] {
    return b"abc";
}

fn relay(bytes: [u8; 3]) -> [u8; 3] {
    return bytes;
}

fn sum(bytes: [u8; 3]) -> i32 {
    let first = bytes[0];
    let second = bytes[1];
    let third = bytes[2];
    return first.to_i32() + second.to_i32() + third.to_i32();
}

fn check_pass_return_move() -> i32 {
    let made = make();
    let relayed = relay(made);
    let moved = relayed;
    let expected = [byte(97), byte(98), byte(99)];
    if !equal_bytes(&moved, &expected) { return -11; }
    if sum(moved) != 294 { return -12; }
    if sum(b"\x01\x02\x03") != 6 { return -13; }
    let mut changed = b"xyz";
    changed[0] = byte(65);
    let expected_changed = [byte(65), byte(121), byte(122)];
    if !equal_bytes(&changed, &expected_changed) { return -14; }
    return 0;
}

fn check_text_accepts_ascii() -> i32 {
    let greeting = b"Hi\n\0";
    let mut owner = new();
    if !grew(try_extend(&mut owner, &greeting)) || len(&owner) != 4 { return -15; }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(rejected) => { return -16; },
        FromBytes::Ok(document) => {
            if byte_len(&document) != 4 || scalar_count(&document) != 4 || !equals_utf8(&document, &greeting) { return -17; }
        },
    }
    return 0;
}

fn check_text_accepts_escaped_utf8() -> i32 {
    let cent = b"\xC2\xA2";
    if cent.len() != 2 || !is_valid(&cent) { return -18; }
    let mut owner = new();
    if !grew(try_extend(&mut owner, &cent)) { return -19; }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(rejected) => { return -20; },
        FromBytes::Ok(document) => {
            if byte_len(&document) != 2 || scalar_count(&document) != 1 || !equals_utf8(&document, &cent) { return -21; }
        },
    }
    return 0;
}

fn check_text_rejects_ff() -> i32 {
    let invalid = b"\xFF";
    let expected = [byte(255)];
    if invalid.len() != 1 || !equal_bytes(&invalid, &expected) { return -22; }
    if !invalid_at(&invalid, 0) { return -23; }
    let truncated = b"A\xC3";
    if !invalid_at(&truncated, 1) { return -24; }
    let mut owner = new();
    if !grew(try_extend(&mut owner, &invalid)) { return -25; }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Ok(unexpected) => { return -26; },
        FromBytes::Invalid(original) => {
            if len(&original) != 1 || !equal_bytes(&original, &expected) { return -27; }
        },
    }
    return 0;
}

fn main() -> i32 {
    let ascii_result = check_ascii();
    if ascii_result != 0 { return ascii_result; }
    let empty_result = check_empty();
    if empty_result != 0 { return empty_result; }
    let high_result = check_nul_and_high_bytes();
    if high_result != 0 { return high_result; }
    let escape_result = check_short_escapes();
    if escape_result != 0 { return escape_result; }
    let hex_result = check_hex_case();
    if hex_result != 0 { return hex_result; }
    let move_result = check_pass_return_move();
    if move_result != 0 { return move_result; }
    let ascii_text_result = check_text_accepts_ascii();
    if ascii_text_result != 0 { return ascii_text_result; }
    let utf8_text_result = check_text_accepts_escaped_utf8();
    if utf8_text_result != 0 { return utf8_text_result; }
    let rejected_text_result = check_text_rejects_ff();
    if rejected_text_result != 0 { return rejected_text_result; }
    return 42;
}
