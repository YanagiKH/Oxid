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

use crate::text::ScalarAppendStatus;
use crate::text::append_scalar;
use crate::text::equals_utf8;
use crate::text::append_to_bytes;

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

fn scalar_appended(document: &mut Text, scalar: i32, expected: &[u8], count: i32) -> bool {
    let status = append_scalar(&mut *document, scalar);
    match status {
        ScalarAppendStatus::Ok => {},
        ScalarAppendStatus::InvalidScalar(value) => { return false; },
        ScalarAppendStatus::CapacityOverflow => { return false; },
        ScalarAppendStatus::AllocationFailed => { return false; },
        ScalarAppendStatus::MemoryLimit => { return false; },
    }
    return byte_len(&*document) == expected.len() && scalar_count(&*document) == count && equals_utf8(&*document, &*expected) && is_valid(&*expected);
}

fn check_scalar_low_edges() -> i32 {
    let mut document = new_text();
    let expected = [byte(0), byte(127), byte(194), byte(128), byte(223), byte(191), byte(224), byte(160), byte(128)];
    if !scalar_appended(&mut document, 0, &expected[0..1], 1) { return -34; }
    if !scalar_appended(&mut document, 127, &expected[0..2], 2) { return -35; }
    if !scalar_appended(&mut document, 128, &expected[0..4], 3) { return -36; }
    if !scalar_appended(&mut document, 2047, &expected[0..6], 4) { return -37; }
    if !scalar_appended(&mut document, 2048, &expected, 5) { return -38; }
    if truncate(&mut document, 8) || !equals_utf8(&document, &expected) { return -39; }
    if !truncate(&mut document, 6) || !equals_utf8(&document, &expected[0..6]) || scalar_count(&document) != 4 { return -40; }
    return 0;
}

fn check_scalar_high_edges() -> i32 {
    let mut document = new_text();
    let expected = [byte(237), byte(159), byte(191), byte(238), byte(128), byte(128), byte(239), byte(191), byte(191), byte(240), byte(144), byte(128), byte(128), byte(244), byte(143), byte(191), byte(191)];
    if !scalar_appended(&mut document, 55295, &expected[0..3], 1) { return -41; }
    if !scalar_appended(&mut document, 57344, &expected[0..6], 2) { return -42; }
    if !scalar_appended(&mut document, 65535, &expected[0..9], 3) { return -43; }
    if !scalar_appended(&mut document, 65536, &expected[0..13], 4) { return -44; }
    if !scalar_appended(&mut document, 1114111, &expected, 5) { return -45; }
    if truncate(&mut document, 16) || !equals_utf8(&document, &expected) { return -46; }
    if !truncate(&mut document, 13) || !equals_utf8(&document, &expected[0..13]) || scalar_count(&document) != 4 { return -47; }
    return 0;
}

fn scalar_refused(document: &mut Text, scalar: i32, expected: &[u8]) -> bool {
    let status = append_scalar(&mut *document, scalar);
    match status {
        ScalarAppendStatus::Ok => { return false; },
        ScalarAppendStatus::InvalidScalar(value) => {
            if value != scalar { return false; }
        },
        ScalarAppendStatus::CapacityOverflow => { return false; },
        ScalarAppendStatus::AllocationFailed => { return false; },
        ScalarAppendStatus::MemoryLimit => { return false; },
    }
    return byte_len(&*document) == expected.len() && scalar_count(&*document) == 1 && equals_utf8(&*document, &*expected) && is_valid(&*expected);
}

fn check_scalar_refusals() -> i32 {
    let mut document = new_text();
    let expected = [byte(240), byte(144), byte(128), byte(128), byte(65)];
    if !scalar_appended(&mut document, 65536, &expected[0..4], 1) { return -48; }
    if !scalar_refused(&mut document, -1, &expected[0..4]) { return -49; }
    if !scalar_refused(&mut document, -2147483647 - 1, &expected[0..4]) { return -50; }
    if !scalar_refused(&mut document, 55296, &expected[0..4]) { return -51; }
    if !scalar_refused(&mut document, 56319, &expected[0..4]) { return -52; }
    if !scalar_refused(&mut document, 56320, &expected[0..4]) { return -53; }
    if !scalar_refused(&mut document, 57343, &expected[0..4]) { return -54; }
    if !scalar_refused(&mut document, 1114112, &expected[0..4]) { return -55; }
    if !scalar_refused(&mut document, 2147483647, &expected[0..4]) { return -56; }
    if !scalar_appended(&mut document, 65, &expected, 2) { return -57; }
    return 0;
}

fn check_export() -> i32 {
    let expected = [byte(0), byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128)];
    let mut owner = new();
    if !grew(try_extend(&mut owner, &expected)) { return -58; }
    let converted = from_bytes(owner);
    match converted {
        FromBytes::Invalid(rejected) => { return -59; },
        FromBytes::Ok(document) => {
            let mut output = new();
            if !grew(append_to_bytes(&document, &mut output)) { return -60; }
            if !equal_bytes(&output, &expected) { return -61; }
            if !equals_utf8(&document, &expected) || byte_len(&document) != 11 || scalar_count(&document) != 5 { return -62; }
            let prefixed_result = check_prefixed_export(&document, &expected);
            if prefixed_result != 0 { return prefixed_result; }
        },
    }
    return 0;
}

fn check_empty_export() -> i32 {
    let document = new_text();
    let mut output = new();
    if !grew(append_to_bytes(&document, &mut output)) || len(&output) != 0 { return -67; }
    let prefix = [byte(255), byte(0)];
    if !grew(try_extend(&mut output, &prefix)) { return -68; }
    if !grew(append_to_bytes(&document, &mut output)) || !equal_bytes(&output, &prefix) { return -69; }
    if byte_len(&document) != 0 || scalar_count(&document) != 0 { return -70; }
    return 0;
}

fn check_prefixed_export(document: &Text, expected: &[u8]) -> i32 {
    let prefix = [byte(255), byte(0)];
    let combined = [byte(255), byte(0), byte(0), byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128)];
    let mut prefixed = new();
    if !grew(try_extend(&mut prefixed, &prefix)) { return -63; }
    if !grew(append_to_bytes(&*document, &mut prefixed)) { return -64; }
    if !equal_bytes(&prefixed, &combined) { return -65; }
    if !equals_utf8(&*document, &*expected) || byte_len(&*document) != 11 || scalar_count(&*document) != 5 { return -66; }
    return 0;
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
    let scalar_low_result = check_scalar_low_edges();
    if scalar_low_result != 0 { return scalar_low_result; }
    let scalar_high_result = check_scalar_high_edges();
    if scalar_high_result != 0 { return scalar_high_result; }
    let scalar_refusal_result = check_scalar_refusals();
    if scalar_refusal_result != 0 { return scalar_refusal_result; }
    let export_result = check_export();
    if export_result != 0 { return export_result; }
    let empty_export_result = check_empty_export();
    if empty_export_result != 0 { return empty_export_result; }
    return 0;
}
