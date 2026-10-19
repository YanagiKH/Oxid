use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use alloc::bytes::truncate as truncate_bytes;
use text::Text;
use text::FromBytes;
use text::AppendStatus;
use text::SliceResult;
use text::new_text;
use text::from_bytes;
use text::byte_len;
use text::scalar_count;
use text::equals_utf8;
use text::append_utf8;
use text::truncate;
use text::into_bytes;
use text::slice;

// Client contracts for `text::slice`. Every expected byte sequence below is a
// fixed raw array written by hand; no expectation is derived from a package
// result or from slicing the corpus.
//
// Mixed corpus (12 bytes, 6 scalars), byte offsets:
//   0 'A' | 1 NUL | 2..4 U+00A2 | 4..7 U+20AC | 7..11 U+1F600 | 11 'z'
// Scalar boundaries: 0 1 2 4 7 11 12. Interior offsets: 3 5 6 8 9 10.

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn mixed() -> [u8; 12] {
    return [byte(65), byte(0), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
}

fn ascii_nul() -> [u8; 5] {
    return [byte(79), byte(120), byte(0), byte(105), byte(100)];
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

fn owned(input: &[u8]) -> FromBytes {
    let mut owner = new();
    let filled = try_extend(&mut owner, &*input);
    if !grew(filled) {
        return FromBytes::Invalid(owner);
    }
    return from_bytes(owner);
}

// Success must be an owned Text whose consumed bytes equal `expected` exactly.
fn sliced_exactly(document: &Text, start: i32, end: i32, expected: &[u8]) -> bool {
    let result = slice(&*document, start, end);
    match result {
        SliceResult::Ok(piece) => {
            if byte_len(&piece) != expected.len() {
                return false;
            }
            let recovered = into_bytes(piece);
            return len(&recovered) == expected.len() && same_bytes(&recovered, &*expected);
        },
        SliceResult::InvalidRange => { return false; },
        SliceResult::CapacityOverflow => { return false; },
        SliceResult::AllocationFailed => { return false; },
        SliceResult::MemoryLimit => { return false; },
    }
}

fn rejected(document: &Text, start: i32, end: i32) -> bool {
    let result = slice(&*document, start, end);
    match result {
        SliceResult::Ok(unexpected) => { return false; },
        SliceResult::InvalidRange => { return true; },
        SliceResult::CapacityOverflow => { return false; },
        SliceResult::AllocationFailed => { return false; },
        SliceResult::MemoryLimit => { return false; },
    }
}

fn unchanged_mixed(document: &Text) -> bool {
    let expected = [byte(65), byte(0), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    return byte_len(&*document) == 12 && scalar_count(&*document) == 6 && equals_utf8(&*document, &expected);
}

fn test_full_range_copies_every_byte() -> i32 {
    let corpus = mixed();
    let whole = [byte(65), byte(0), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 101; },
        FromBytes::Ok(document) => {
            if !sliced_exactly(&document, 0, 12, &whole) { return 102; }
            if !unchanged_mixed(&document) { return 103; }
        },
    }
    let plain = ascii_nul();
    let plain_whole = [byte(79), byte(120), byte(0), byte(105), byte(100)];
    let plain_converted = owned(&plain);
    match plain_converted {
        FromBytes::Invalid(unexpected_plain_owner) => { return 104; },
        FromBytes::Ok(plain_document) => {
            if !sliced_exactly(&plain_document, 0, 5, &plain_whole) { return 105; }
            if !equals_utf8(&plain_document, &plain_whole) { return 106; }
        },
    }
    return 0;
}

fn test_prefix_middle_suffix_ranges() -> i32 {
    let corpus = mixed();
    let prefix = [byte(65), byte(0), byte(194), byte(162)];
    let middle = [byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128)];
    let suffix = [byte(240), byte(159), byte(152), byte(128), byte(122)];
    let inner = [byte(0), byte(194), byte(162), byte(226), byte(130), byte(172)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 201; },
        FromBytes::Ok(document) => {
            if !sliced_exactly(&document, 0, 4, &prefix) { return 202; }
            if !sliced_exactly(&document, 2, 11, &middle) { return 203; }
            if !sliced_exactly(&document, 7, 12, &suffix) { return 204; }
            if !sliced_exactly(&document, 1, 7, &inner) { return 205; }
            if !unchanged_mixed(&document) { return 206; }
        },
    }
    return 0;
}

fn test_each_scalar_width_alone() -> i32 {
    let corpus = mixed();
    let ascii = [byte(65)];
    let nul = [byte(0)];
    let two = [byte(194), byte(162)];
    let three = [byte(226), byte(130), byte(172)];
    let four = [byte(240), byte(159), byte(152), byte(128)];
    let last = [byte(122)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 301; },
        FromBytes::Ok(document) => {
            if !sliced_exactly(&document, 0, 1, &ascii) { return 302; }
            if !sliced_exactly(&document, 1, 2, &nul) { return 303; }
            if !sliced_exactly(&document, 2, 4, &two) { return 304; }
            if !sliced_exactly(&document, 4, 7, &three) { return 305; }
            if !sliced_exactly(&document, 7, 11, &four) { return 306; }
            if !sliced_exactly(&document, 11, 12, &last) { return 307; }
            if !unchanged_mixed(&document) { return 308; }
        },
    }
    return 0;
}

fn test_ascii_and_nul_subranges() -> i32 {
    let plain = ascii_nul();
    let around_nul = [byte(120), byte(0), byte(105)];
    let only_nul = [byte(0)];
    let tail = [byte(105), byte(100)];
    let plain_whole = [byte(79), byte(120), byte(0), byte(105), byte(100)];
    let converted = owned(&plain);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 401; },
        FromBytes::Ok(document) => {
            if !sliced_exactly(&document, 1, 4, &around_nul) { return 402; }
            if !sliced_exactly(&document, 2, 3, &only_nul) { return 403; }
            if !sliced_exactly(&document, 3, 5, &tail) { return 404; }
            if byte_len(&document) != 5 || scalar_count(&document) != 5 || !equals_utf8(&document, &plain_whole) { return 405; }
        },
    }
    return 0;
}

fn test_empty_ranges_on_boundaries() -> i32 {
    let corpus = mixed();
    let none: [u8; 0] = [];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 501; },
        FromBytes::Ok(document) => {
            if !sliced_exactly(&document, 0, 0, &none) { return 502; }
            if !sliced_exactly(&document, 1, 1, &none) { return 503; }
            if !sliced_exactly(&document, 2, 2, &none) { return 504; }
            if !sliced_exactly(&document, 4, 4, &none) { return 505; }
            if !sliced_exactly(&document, 7, 7, &none) { return 506; }
            if !sliced_exactly(&document, 11, 11, &none) { return 507; }
            if !sliced_exactly(&document, 12, 12, &none) { return 508; }
            if !unchanged_mixed(&document) { return 509; }
        },
    }
    let empty = new_text();
    if !sliced_exactly(&empty, 0, 0, &none) { return 510; }
    if byte_len(&empty) != 0 { return 511; }
    return 0;
}

fn test_interior_start_rejected() -> i32 {
    let corpus = mixed();
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 601; },
        FromBytes::Ok(document) => {
            if !rejected(&document, 3, 12) { return 602; }
            if !rejected(&document, 5, 12) { return 603; }
            if !rejected(&document, 6, 12) { return 604; }
            if !rejected(&document, 8, 12) { return 605; }
            if !rejected(&document, 9, 12) { return 606; }
            if !rejected(&document, 10, 12) { return 607; }
            if !rejected(&document, 3, 4) { return 608; }
            if !rejected(&document, 6, 7) { return 609; }
            if !rejected(&document, 10, 11) { return 610; }
            if !unchanged_mixed(&document) { return 611; }
        },
    }
    return 0;
}

fn test_interior_end_rejected() -> i32 {
    let corpus = mixed();
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 701; },
        FromBytes::Ok(document) => {
            if !rejected(&document, 0, 3) { return 702; }
            if !rejected(&document, 0, 5) { return 703; }
            if !rejected(&document, 0, 6) { return 704; }
            if !rejected(&document, 0, 8) { return 705; }
            if !rejected(&document, 0, 9) { return 706; }
            if !rejected(&document, 0, 10) { return 707; }
            if !rejected(&document, 2, 3) { return 708; }
            if !rejected(&document, 4, 5) { return 709; }
            if !rejected(&document, 7, 10) { return 710; }
            if !unchanged_mixed(&document) { return 711; }
        },
    }
    return 0;
}

fn test_interior_empty_and_both_interior_rejected() -> i32 {
    let corpus = mixed();
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 801; },
        FromBytes::Ok(document) => {
            if !rejected(&document, 3, 3) { return 802; }
            if !rejected(&document, 5, 5) { return 803; }
            if !rejected(&document, 6, 6) { return 804; }
            if !rejected(&document, 8, 8) { return 805; }
            if !rejected(&document, 9, 9) { return 806; }
            if !rejected(&document, 10, 10) { return 807; }
            if !rejected(&document, 3, 5) { return 808; }
            if !rejected(&document, 5, 9) { return 809; }
            if !unchanged_mixed(&document) { return 810; }
        },
    }
    return 0;
}

fn test_negative_bounds_rejected() -> i32 {
    let corpus = mixed();
    let lowest = -2147483647 - 1;
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 901; },
        FromBytes::Ok(document) => {
            if !rejected(&document, -1, 0) { return 902; }
            if !rejected(&document, -1, 4) { return 903; }
            if !rejected(&document, 0, -1) { return 904; }
            if !rejected(&document, -1, -1) { return 905; }
            if !rejected(&document, lowest, 0) { return 906; }
            if !rejected(&document, lowest, 12) { return 907; }
            if !rejected(&document, 4, lowest) { return 908; }
            if !unchanged_mixed(&document) { return 909; }
        },
    }
    let empty = new_text();
    if !rejected(&empty, -1, 0) { return 910; }
    return 0;
}

fn test_reversed_bounds_rejected() -> i32 {
    let corpus = mixed();
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1001; },
        FromBytes::Ok(document) => {
            if !rejected(&document, 1, 0) { return 1002; }
            if !rejected(&document, 4, 2) { return 1003; }
            if !rejected(&document, 11, 7) { return 1004; }
            if !rejected(&document, 12, 11) { return 1005; }
            if !rejected(&document, 12, 0) { return 1006; }
            if !unchanged_mixed(&document) { return 1007; }
        },
    }
    return 0;
}

fn test_out_of_range_bounds_rejected() -> i32 {
    let corpus = mixed();
    let highest = 2147483647;
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1101; },
        FromBytes::Ok(document) => {
            if !rejected(&document, 0, 13) { return 1102; }
            if !rejected(&document, 12, 13) { return 1103; }
            if !rejected(&document, 13, 13) { return 1104; }
            if !rejected(&document, 13, 14) { return 1105; }
            if !rejected(&document, 0, highest) { return 1106; }
            if !rejected(&document, highest, highest) { return 1107; }
            if !unchanged_mixed(&document) { return 1108; }
        },
    }
    let empty = new_text();
    if !rejected(&empty, 0, 1) { return 1109; }
    if !rejected(&empty, 1, 1) { return 1110; }
    return 0;
}

fn test_changing_result_text_keeps_original() -> i32 {
    let corpus = mixed();
    let cent = [byte(194), byte(162)];
    let tail = [byte(122)];
    let cent_z = [byte(194), byte(162), byte(122)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1201; },
        FromBytes::Ok(document) => {
            let result = slice(&document, 2, 11);
            match result {
                SliceResult::Ok(piece) => {
                    let mut changed = piece;
                    if !truncate(&mut changed, 2) { return 1202; }
                    if !equals_utf8(&changed, &cent) { return 1203; }
                    if !unchanged_mixed(&document) { return 1204; }
                    let appended = append_utf8(&mut changed, &tail);
                    match appended {
                        AppendStatus::Ok => {},
                        AppendStatus::InvalidInput(offset) => { return 1205; },
                        AppendStatus::CapacityOverflow => { return 1206; },
                        AppendStatus::AllocationFailed => { return 1207; },
                        AppendStatus::MemoryLimit => { return 1208; },
                    }
                    if !equals_utf8(&changed, &cent_z) { return 1209; }
                    if !unchanged_mixed(&document) { return 1210; }
                },
                SliceResult::InvalidRange => { return 1211; },
                SliceResult::CapacityOverflow => { return 1212; },
                SliceResult::AllocationFailed => { return 1213; },
                SliceResult::MemoryLimit => { return 1214; },
            }
        },
    }
    return 0;
}

fn test_changing_result_bytes_keeps_original() -> i32 {
    let corpus = mixed();
    let euro = [byte(226), byte(130), byte(172)];
    let euro_head = [byte(226)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1301; },
        FromBytes::Ok(document) => {
            let result = slice(&document, 4, 7);
            match result {
                SliceResult::Ok(piece) => {
                    let mut recovered = into_bytes(piece);
                    if len(&recovered) != 3 || !same_bytes(&recovered, &euro) { return 1302; }
                    truncate_bytes(&mut recovered, 1);
                    if !same_bytes(&recovered, &euro_head) { return 1303; }
                    if !unchanged_mixed(&document) { return 1304; }
                    if !grew(try_extend(&mut recovered, &corpus)) { return 1305; }
                    if len(&recovered) != 13 { return 1306; }
                    if !unchanged_mixed(&document) { return 1307; }
                },
                SliceResult::InvalidRange => { return 1308; },
                SliceResult::CapacityOverflow => { return 1309; },
                SliceResult::AllocationFailed => { return 1310; },
                SliceResult::MemoryLimit => { return 1311; },
            }
        },
    }
    return 0;
}

fn test_changing_original_keeps_result() -> i32 {
    let corpus = mixed();
    let suffix = [byte(240), byte(159), byte(152), byte(128), byte(122)];
    let head = [byte(65)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1401; },
        FromBytes::Ok(document) => {
            let mut source = document;
            let result = slice(&source, 7, 12);
            match result {
                SliceResult::Ok(piece) => {
                    if !truncate(&mut source, 1) { return 1402; }
                    if !equals_utf8(&source, &head) { return 1403; }
                    if byte_len(&piece) != 5 || scalar_count(&piece) != 2 || !equals_utf8(&piece, &suffix) { return 1404; }
                },
                SliceResult::InvalidRange => { return 1405; },
                SliceResult::CapacityOverflow => { return 1406; },
                SliceResult::AllocationFailed => { return 1407; },
                SliceResult::MemoryLimit => { return 1408; },
            }
        },
    }
    return 0;
}

fn main() -> i32 {
    let full = test_full_range_copies_every_byte();
    if full != 0 { return full; }
    let ranges = test_prefix_middle_suffix_ranges();
    if ranges != 0 { return ranges; }
    let widths = test_each_scalar_width_alone();
    if widths != 0 { return widths; }
    let plain = test_ascii_and_nul_subranges();
    if plain != 0 { return plain; }
    let empty = test_empty_ranges_on_boundaries();
    if empty != 0 { return empty; }
    let interior_start = test_interior_start_rejected();
    if interior_start != 0 { return interior_start; }
    let interior_end = test_interior_end_rejected();
    if interior_end != 0 { return interior_end; }
    let interior_empty = test_interior_empty_and_both_interior_rejected();
    if interior_empty != 0 { return interior_empty; }
    let negative = test_negative_bounds_rejected();
    if negative != 0 { return negative; }
    let reversed = test_reversed_bounds_rejected();
    if reversed != 0 { return reversed; }
    let overlong = test_out_of_range_bounds_rejected();
    if overlong != 0 { return overlong; }
    let result_text = test_changing_result_text_keeps_original();
    if result_text != 0 { return result_text; }
    let result_bytes = test_changing_result_bytes_keeps_original();
    if result_bytes != 0 { return result_bytes; }
    let original = test_changing_original_keeps_result();
    if original != 0 { return original; }
    return 42;
}
