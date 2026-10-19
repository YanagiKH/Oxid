mod text;

use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use crate::text::Text;
use crate::text::Validation;
use crate::text::FromBytes;
use crate::text::AppendStatus;
use crate::text::validate;
use crate::text::new_text;
use crate::text::from_bytes;
use crate::text::byte_len;
use crate::text::scalar_count;
use crate::text::append_utf8;
use crate::text::truncate;

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
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

fn check_empty() -> i32 {
    let empty_input: [u8; 0] = [];
    let empty_text = new_text();
    if !is_valid(&empty_input) || byte_len(&empty_text) != 0 || scalar_count(&empty_text) != 0 {
        return -1;
    }
    return 0;
}

fn check_valid_edges() -> i32 {
    let edges = [byte(0), byte(127), byte(194), byte(128), byte(223), byte(191), byte(224), byte(160), byte(128), byte(237), byte(159), byte(191), byte(239), byte(191), byte(191), byte(240), byte(144), byte(128), byte(128), byte(244), byte(143), byte(191), byte(191)];
    if !is_valid(&edges) {
        return -2;
    }
    return 0;
}

fn check_invalid_leads() -> i32 {
    let isolated = [byte(128)];
    let overlong_two = [byte(192), byte(128)];
    let overlong_three = [byte(224), byte(159), byte(191)];
    let surrogate = [byte(237), byte(160), byte(128)];
    let overlong_four = [byte(240), byte(143), byte(191), byte(191)];
    let too_high = [byte(244), byte(144), byte(128), byte(128)];
    let forbidden_lead = [byte(245), byte(128), byte(128), byte(128)];
    if !invalid_at(&isolated, 0) || !invalid_at(&overlong_two, 0) || !invalid_at(&overlong_three, 0) || !invalid_at(&surrogate, 0) || !invalid_at(&overlong_four, 0) || !invalid_at(&too_high, 0) || !invalid_at(&forbidden_lead, 0) {
        return -3;
    }
    return 0;
}

fn check_invalid_offsets() -> i32 {
    let invalid_input = [byte(65), byte(226), byte(40), byte(161)];
    let after_multibyte = [byte(194), byte(162), byte(226), byte(40), byte(161)];
    let bad_third = [byte(226), byte(130), byte(65)];
    let bad_fourth = [byte(240), byte(159), byte(152), byte(65)];
    let truncated_two = [byte(65), byte(194)];
    let truncated_three = [byte(65), byte(226), byte(130)];
    let truncated_four = [byte(65), byte(240), byte(159), byte(152)];
    if !invalid_at(&invalid_input, 1) || !invalid_at(&after_multibyte, 2) || !invalid_at(&bad_third, 0) || !invalid_at(&bad_fourth, 0) || !invalid_at(&truncated_two, 1) || !invalid_at(&truncated_three, 1) || !invalid_at(&truncated_four, 1) {
        return -4;
    }
    return 0;
}

fn check_invalid_conversion() -> i32 {
    let invalid_input = [byte(65), byte(226), byte(40), byte(161)];
    let mut rejected_owner = new();
    let rejected_growth = try_extend(&mut rejected_owner, &invalid_input);
    if !grew(rejected_growth) {
        return -5;
    }
    let rejected = from_bytes(rejected_owner);
    match rejected {
        FromBytes::Ok(unexpected_text) => { return -6; },
        FromBytes::Invalid(original_owner) => {
            if len(&original_owner) != 4 || !equal_bytes(&original_owner, &invalid_input) {
                return -7;
            }
        },
    }
    return 0;
}

fn check_append(document: &mut Text) -> i32 {
    if byte_len(&*document) != 10 || scalar_count(&*document) != 4 {
        return -10;
    }
    let suffix = [byte(0), byte(101), byte(204), byte(129)];
    let appended = append_utf8(&mut *document, &suffix);
    match appended {
        AppendStatus::Ok => {},
        AppendStatus::InvalidInput(suffix_error) => { return -11; },
        AppendStatus::CapacityOverflow => { return -12; },
        AppendStatus::AllocationFailed => { return -13; },
        AppendStatus::MemoryLimit => { return -14; },
    }
    if byte_len(&*document) != 14 || scalar_count(&*document) != 7 {
        return -15;
    }
    return 0;
}

fn check_invalid_append(document: &mut Text) -> i32 {
    let invalid_input = [byte(65), byte(226), byte(40), byte(161)];
    let refused = append_utf8(&mut *document, &invalid_input);
    match refused {
        AppendStatus::Ok => { return -16; },
        AppendStatus::InvalidInput(append_error) => {
            if append_error != 1 { return -17; }
        },
        AppendStatus::CapacityOverflow => { return -18; },
        AppendStatus::AllocationFailed => { return -19; },
        AppendStatus::MemoryLimit => { return -20; },
    }
    if byte_len(&*document) != 14 || scalar_count(&*document) != 7 {
        return -21;
    }
    return 0;
}

fn check_empty_append(document: &mut Text) -> i32 {
    let empty_input: [u8; 0] = [];
    let appended_empty = append_utf8(&mut *document, &empty_input);
    match appended_empty {
        AppendStatus::Ok => {},
        AppendStatus::InvalidInput(empty_error) => { return -22; },
        AppendStatus::CapacityOverflow => { return -23; },
        AppendStatus::AllocationFailed => { return -24; },
        AppendStatus::MemoryLimit => { return -25; },
    }
    return 0;
}

fn check_rejected_truncation(document: &mut Text) -> i32 {
    if truncate(&mut *document, -1) || truncate(&mut *document, 15) || truncate(&mut *document, 13) || truncate(&mut *document, 2) {
        return -26;
    }
    if byte_len(&*document) != 14 || scalar_count(&*document) != 7 {
        return -27;
    }
    return 0;
}

fn check_boundary_truncation(document: &mut Text) -> i32 {
    if !truncate(&mut *document, 14) || !truncate(&mut *document, 12) {
        return -28;
    }
    if byte_len(&*document) != 12 || scalar_count(&*document) != 6 {
        return -29;
    }
    if !truncate(&mut *document, 6) {
        return -30;
    }
    if byte_len(&*document) != 6 || scalar_count(&*document) != 3 {
        return -31;
    }
    if !truncate(&mut *document, 0) {
        return -32;
    }
    if byte_len(&*document) != 0 || scalar_count(&*document) != 0 {
        return -33;
    }
    return 0;
}

fn check_valid_conversion() -> i32 {
    let mixed = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128)];
    let mut accepted_owner = new();
    let accepted_growth = try_extend(&mut accepted_owner, &mixed);
    if !grew(accepted_growth) {
        return -8;
    }
    let accepted = from_bytes(accepted_owner);
    match accepted {
        FromBytes::Invalid(unexpected_owner) => { return -9; },
        FromBytes::Ok(owned_text) => {
            let mut document = owned_text;
            let append_result = check_append(&mut document);
            if append_result != 0 { return append_result; }
            let invalid_result = check_invalid_append(&mut document);
            if invalid_result != 0 { return invalid_result; }
            let empty_result = check_empty_append(&mut document);
            if empty_result != 0 { return empty_result; }
            let rejected_boundary_result = check_rejected_truncation(&mut document);
            if rejected_boundary_result != 0 { return rejected_boundary_result; }
            let accepted_boundary_result = check_boundary_truncation(&mut document);
            if accepted_boundary_result != 0 { return accepted_boundary_result; }
            return 0;
        },
    }
}

fn main() -> i32 {
    let empty_result = check_empty();
    if empty_result != 0 { return empty_result; }
    let edges_result = check_valid_edges();
    if edges_result != 0 { return edges_result; }
    let leads_result = check_invalid_leads();
    if leads_result != 0 { return leads_result; }
    let offsets_result = check_invalid_offsets();
    if offsets_result != 0 { return offsets_result; }
    let rejected_result = check_invalid_conversion();
    if rejected_result != 0 { return rejected_result; }
    let accepted_result = check_valid_conversion();
    if accepted_result != 0 { return accepted_result; }
    return 42;
}
