use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use text::Text;
use text::Validation;
use text::FromBytes;
use text::validate;
use text::from_bytes;
use text::equals_utf8;
use text::append_to_bytes;

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn same_bytes(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut offset = 0;
    while offset < expected.len() {
        if actual[offset] != expected[offset] {
            return false;
        }
        offset = offset + 1;
    }
    return true;
}

fn is_valid(input: &[u8]) -> bool {
    let checked = validate(&*input);
    match checked {
        Validation::Valid => { return true; },
        Validation::Invalid(offset) => { return false; },
    }
}

fn copied_exactly(document: &Text, prefix: &[u8], expected: &[u8]) -> bool {
    let mut destination = new();
    let prefixed = try_extend(&mut destination, &*prefix);
    if !grew(prefixed) {
        return false;
    }
    let appended = append_to_bytes(&*document, &mut destination);
    if !grew(appended) {
        return false;
    }
    return same_bytes(&destination, &*expected);
}

fn round_trips(input: &[u8]) -> i32 {
    let mut owner = new();
    let filled = try_extend(&mut owner, &*input);
    if !grew(filled) {
        return 1;
    }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 2; },
        FromBytes::Ok(document) => {
            let empty = [byte(0)];
            if !copied_exactly(&document, &empty[0..0], &*input) {
                return 3;
            }
            if !equals_utf8(&document, &*input) {
                return 4;
            }
            return 0;
        },
    }
}

fn rejected_unchanged(input: &[u8]) -> bool {
    if is_valid(&*input) {
        return false;
    }
    let mut owner = new();
    let filled = try_extend(&mut owner, &*input);
    if !grew(filled) {
        return false;
    }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Ok(unexpected_text) => { return false; },
        FromBytes::Invalid(original_owner) => {
            return len(&original_owner) == input.len() && same_bytes(&original_owner, &*input);
        },
    }
}

fn test_append_keeps_text_and_destination_prefix() -> i32 {
    let input = [byte(65), byte(0), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(101), byte(204), byte(129)];
    let prefixed = [byte(80), byte(65), byte(0), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(101), byte(204), byte(129)];
    let mut owner = new();
    let filled = try_extend(&mut owner, &input);
    if !grew(filled) {
        return 11;
    }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 12; },
        FromBytes::Ok(document) => {
            if !copied_exactly(&document, &prefixed[0..1], &prefixed) {
                return 13;
            }
            if !equals_utf8(&document, &input) {
                return 14;
            }
            return 0;
        },
    }
}

fn test_append_twice_is_exact() -> i32 {
    let input = [byte(239), byte(187), byte(191), byte(66), byte(10)];
    let doubled = [byte(239), byte(187), byte(191), byte(66), byte(10), byte(239), byte(187), byte(191), byte(66), byte(10)];
    let mut owner = new();
    let filled = try_extend(&mut owner, &input);
    if !grew(filled) {
        return 21;
    }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 22; },
        FromBytes::Ok(document) => {
            let mut destination = new();
            let first = append_to_bytes(&document, &mut destination);
            if !grew(first) {
                return 23;
            }
            let second = append_to_bytes(&document, &mut destination);
            if !grew(second) {
                return 24;
            }
            if !same_bytes(&destination, &doubled) || !equals_utf8(&document, &input) {
                return 25;
            }
            return 0;
        },
    }
}

fn test_empty_text_appends_nothing() -> i32 {
    let prefix = [byte(80)];
    let owner = new();
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 31; },
        FromBytes::Ok(document) => {
            if !copied_exactly(&document, &prefix, &prefix) {
                return 32;
            }
            if !equals_utf8(&document, &prefix[0..0]) {
                return 33;
            }
            return 0;
        },
    }
}

fn test_valid_classes_round_trip() -> i32 {
    let ascii = [byte(79), byte(120), byte(105), byte(100), byte(10)];
    let nul = [byte(0), byte(65), byte(0), byte(0), byte(66), byte(10), byte(0)];
    let edges = [byte(65), byte(194), byte(128), byte(223), byte(191), byte(224), byte(160), byte(128), byte(237), byte(159), byte(191), byte(238), byte(128), byte(128), byte(239), byte(191), byte(191), byte(240), byte(144), byte(128), byte(128), byte(244), byte(143), byte(191), byte(191), byte(10)];
    let combining = [byte(101), byte(204), byte(129), byte(32), byte(97), byte(204), byte(136), byte(204), byte(163), byte(10)];
    let bom = [byte(239), byte(187), byte(191), byte(66), byte(79), byte(77), byte(10)];
    let ascii_result = round_trips(&ascii);
    if ascii_result != 0 { return 40 + ascii_result; }
    let nul_result = round_trips(&nul);
    if nul_result != 0 { return 50 + nul_result; }
    let edges_result = round_trips(&edges);
    if edges_result != 0 { return 60 + edges_result; }
    let combining_result = round_trips(&combining);
    if combining_result != 0 { return 70 + combining_result; }
    let bom_result = round_trips(&bom);
    if bom_result != 0 { return 80 + bom_result; }
    return 0;
}

fn test_invalid_classes_return_original_owner() -> i32 {
    let overlong = [byte(65), byte(192), byte(175), byte(10)];
    let surrogate = [byte(65), byte(237), byte(160), byte(128), byte(10)];
    let above_maximum = [byte(65), byte(244), byte(144), byte(128), byte(128), byte(10)];
    let continuation = [byte(65), byte(128), byte(10)];
    let truncated = [byte(65), byte(226), byte(130)];
    if !rejected_unchanged(&overlong) { return 91; }
    if !rejected_unchanged(&surrogate) { return 92; }
    if !rejected_unchanged(&above_maximum) { return 93; }
    if !rejected_unchanged(&continuation) { return 94; }
    if !rejected_unchanged(&truncated) { return 95; }
    return 0;
}

fn main() -> i32 {
    let prefix_result = test_append_keeps_text_and_destination_prefix();
    if prefix_result != 0 { return prefix_result; }
    let twice_result = test_append_twice_is_exact();
    if twice_result != 0 { return twice_result; }
    let empty_result = test_empty_text_appends_nothing();
    if empty_result != 0 { return empty_result; }
    let valid_result = test_valid_classes_round_trip();
    if valid_result != 0 { return valid_result; }
    let invalid_result = test_invalid_classes_return_original_owner();
    if invalid_result != 0 { return invalid_result; }
    return 42;
}
