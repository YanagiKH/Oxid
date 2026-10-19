use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use text::Text;
use text::FromBytes;
use text::SliceResult;
use text::ScalarStep;
use text::ScalarRead;
use text::new_text;
use text::from_bytes;
use text::byte_len;
use text::scalar_count;
use text::equals_utf8;
use text::into_bytes;
use text::slice;
use text::next_scalar;

// Client contracts for `text::next_scalar`. Every corpus, scalar value, next
// byte offset and token spelling below is written by hand in advance; nothing
// is derived from a package result, from `append_scalar`, or by decoding the
// corpus. The cursor is an ordinary i32 owned by the caller and passed by
// value; the client advances it from `ScalarStep::next_byte_offset`.
//
// Corpora and byte offsets:
//   ascii:   0 NUL | 1 'A' | 2 U+007F                                   (3 bytes)
//   two:     0..2 U+0080 | 2..4 U+07FF                                   (4 bytes)
//   three:   0..3 U+0800 | 3..6 U+D7FF | 6..9 U+E000 | 9..12 U+FFFF       (12 bytes)
//   four:    0..4 U+10000 | 4..8 U+10FFFF                                (8 bytes)
//   special: 0..3 U+FEFF BOM | 3 'e' | 4..6 U+0301 | 6..9 U+FDD0 |
//            9..12 U+FFFE | 12..16 U+1FFFE | 16..20 U+10FFFE             (20 bytes)
//   mixed:   0 'A' | 1..3 U+00A2 | 3..6 U+20AC | 6..10 U+1F600 | 10 'z'   (11 bytes)
//   tokens:  "x1 cafe\u{301} \u{20AC}5 \u{1F600}!"                       (20 bytes)

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn ascii_corpus() -> [u8; 3] {
    return [byte(0), byte(65), byte(127)];
}

fn two_byte_corpus() -> [u8; 4] {
    return [byte(194), byte(128), byte(223), byte(191)];
}

fn three_byte_corpus() -> [u8; 12] {
    return [byte(224), byte(160), byte(128), byte(237), byte(159), byte(191), byte(238), byte(128), byte(128), byte(239), byte(191), byte(191)];
}

fn four_byte_corpus() -> [u8; 8] {
    return [byte(240), byte(144), byte(128), byte(128), byte(244), byte(143), byte(191), byte(191)];
}

fn special_corpus() -> [u8; 20] {
    return [byte(239), byte(187), byte(191), byte(101), byte(204), byte(129), byte(239), byte(183), byte(144), byte(239), byte(191), byte(190), byte(240), byte(159), byte(191), byte(190), byte(244), byte(143), byte(191), byte(190)];
}

fn mixed_corpus() -> [u8; 11] {
    return [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
}

fn token_corpus() -> [u8; 20] {
    return [byte(120), byte(49), byte(32), byte(99), byte(97), byte(102), byte(101), byte(204), byte(129), byte(32), byte(226), byte(130), byte(172), byte(53), byte(32), byte(240), byte(159), byte(152), byte(128), byte(33)];
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

// ScalarStep is plain data: it is taken by value and only its two fields are read.
fn step_is(step: ScalarStep, scalar: i32, next: i32) -> bool {
    return step.scalar == scalar && step.next_byte_offset == next;
}

fn reads_value(document: &Text, offset: i32, scalar: i32, next: i32) -> bool {
    let read = next_scalar(&*document, offset);
    match read {
        ScalarRead::Value(step) => { return step_is(step, scalar, next); },
        ScalarRead::End => { return false; },
        ScalarRead::InvalidOffset => { return false; },
    }
}

fn reads_end(document: &Text, offset: i32) -> bool {
    let read = next_scalar(&*document, offset);
    match read {
        ScalarRead::Value(unexpected) => { return false; },
        ScalarRead::End => { return true; },
        ScalarRead::InvalidOffset => { return false; },
    }
}

fn reads_invalid(document: &Text, offset: i32) -> bool {
    let read = next_scalar(&*document, offset);
    match read {
        ScalarRead::Value(unexpected) => { return false; },
        ScalarRead::End => { return false; },
        ScalarRead::InvalidOffset => { return true; },
    }
}

// The original Text must keep its exact bytes, length and scalar count.
fn unchanged(document: &Text, expected: &[u8], scalars: i32) -> bool {
    return byte_len(&*document) == expected.len() && scalar_count(&*document) == scalars && equals_utf8(&*document, &*expected);
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

fn test_nul_and_ascii_scalars() -> i32 {
    let corpus = ascii_corpus();
    let original = [byte(0), byte(65), byte(127)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 101; },
        FromBytes::Ok(document) => {
            if !reads_value(&document, 0, 0, 1) { return 102; }
            if !reads_value(&document, 1, 65, 2) { return 103; }
            if !reads_value(&document, 2, 127, 3) { return 104; }
            if !reads_end(&document, 3) { return 105; }
            if !unchanged(&document, &original, 3) { return 106; }
        },
    }
    return 0;
}

fn test_two_byte_boundaries() -> i32 {
    let corpus = two_byte_corpus();
    let original = [byte(194), byte(128), byte(223), byte(191)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 201; },
        FromBytes::Ok(document) => {
            if !reads_value(&document, 0, 128, 2) { return 202; }
            if !reads_value(&document, 2, 2047, 4) { return 203; }
            if !reads_end(&document, 4) { return 204; }
            if !reads_invalid(&document, 1) { return 205; }
            if !reads_invalid(&document, 3) { return 206; }
            if !unchanged(&document, &original, 2) { return 207; }
        },
    }
    return 0;
}

fn test_three_byte_boundaries_around_surrogates() -> i32 {
    let corpus = three_byte_corpus();
    let original = [byte(224), byte(160), byte(128), byte(237), byte(159), byte(191), byte(238), byte(128), byte(128), byte(239), byte(191), byte(191)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 301; },
        FromBytes::Ok(document) => {
            if !reads_value(&document, 0, 2048, 3) { return 302; }
            if !reads_value(&document, 3, 55295, 6) { return 303; }
            if !reads_value(&document, 6, 57344, 9) { return 304; }
            if !reads_value(&document, 9, 65535, 12) { return 305; }
            if !reads_end(&document, 12) { return 306; }
            if !reads_invalid(&document, 1) { return 307; }
            if !reads_invalid(&document, 2) { return 308; }
            if !reads_invalid(&document, 4) { return 309; }
            if !reads_invalid(&document, 5) { return 310; }
            if !reads_invalid(&document, 7) { return 311; }
            if !reads_invalid(&document, 8) { return 312; }
            if !reads_invalid(&document, 10) { return 313; }
            if !reads_invalid(&document, 11) { return 314; }
            if !unchanged(&document, &original, 4) { return 315; }
        },
    }
    return 0;
}

fn test_four_byte_boundaries_to_max_scalar() -> i32 {
    let corpus = four_byte_corpus();
    let original = [byte(240), byte(144), byte(128), byte(128), byte(244), byte(143), byte(191), byte(191)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 401; },
        FromBytes::Ok(document) => {
            if !reads_value(&document, 0, 65536, 4) { return 402; }
            if !reads_value(&document, 4, 1114111, 8) { return 403; }
            if !reads_end(&document, 8) { return 404; }
            if !reads_invalid(&document, 1) { return 405; }
            if !reads_invalid(&document, 2) { return 406; }
            if !reads_invalid(&document, 3) { return 407; }
            if !reads_invalid(&document, 5) { return 408; }
            if !reads_invalid(&document, 6) { return 409; }
            if !reads_invalid(&document, 7) { return 410; }
            if !unchanged(&document, &original, 2) { return 411; }
        },
    }
    return 0;
}

fn test_bom_combining_and_noncharacters() -> i32 {
    let corpus = special_corpus();
    let original = [byte(239), byte(187), byte(191), byte(101), byte(204), byte(129), byte(239), byte(183), byte(144), byte(239), byte(191), byte(190), byte(240), byte(159), byte(191), byte(190), byte(244), byte(143), byte(191), byte(190)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 501; },
        FromBytes::Ok(document) => {
            if !reads_value(&document, 0, 65279, 3) { return 502; }
            if !reads_value(&document, 3, 101, 4) { return 503; }
            if !reads_value(&document, 4, 769, 6) { return 504; }
            if !reads_value(&document, 6, 64976, 9) { return 505; }
            if !reads_value(&document, 9, 65534, 12) { return 506; }
            if !reads_value(&document, 12, 131070, 16) { return 507; }
            if !reads_value(&document, 16, 1114110, 20) { return 508; }
            if !reads_end(&document, 20) { return 509; }
            if !reads_invalid(&document, 1) { return 510; }
            if !reads_invalid(&document, 2) { return 511; }
            if !reads_invalid(&document, 5) { return 512; }
            if !unchanged(&document, &original, 7) { return 513; }
        },
    }
    return 0;
}

fn test_every_interior_offset_rejected() -> i32 {
    let corpus = mixed_corpus();
    let original = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 601; },
        FromBytes::Ok(document) => {
            if !reads_invalid(&document, 2) { return 602; }
            if !reads_invalid(&document, 4) { return 603; }
            if !reads_invalid(&document, 5) { return 604; }
            if !reads_invalid(&document, 7) { return 605; }
            if !reads_invalid(&document, 8) { return 606; }
            if !reads_invalid(&document, 9) { return 607; }
            if !reads_value(&document, 0, 65, 1) { return 608; }
            if !reads_value(&document, 1, 162, 3) { return 609; }
            if !reads_value(&document, 3, 8364, 6) { return 610; }
            if !reads_value(&document, 6, 128512, 10) { return 611; }
            if !reads_value(&document, 10, 122, 11) { return 612; }
            if !unchanged(&document, &original, 5) { return 613; }
        },
    }
    return 0;
}

fn test_negative_offsets_rejected() -> i32 {
    let corpus = mixed_corpus();
    let original = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let lowest = -2147483647 - 1;
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 701; },
        FromBytes::Ok(document) => {
            if !reads_invalid(&document, -1) { return 702; }
            if !reads_invalid(&document, -2) { return 703; }
            if !reads_invalid(&document, -11) { return 704; }
            if !reads_invalid(&document, lowest) { return 705; }
            if !unchanged(&document, &original, 5) { return 706; }
        },
    }
    let empty = new_text();
    if !reads_invalid(&empty, -1) { return 707; }
    if !reads_invalid(&empty, lowest) { return 708; }
    if byte_len(&empty) != 0 { return 709; }
    return 0;
}

fn test_offsets_past_end_rejected() -> i32 {
    let corpus = mixed_corpus();
    let original = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let highest = 2147483647;
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 801; },
        FromBytes::Ok(document) => {
            if !reads_invalid(&document, 12) { return 802; }
            if !reads_invalid(&document, 13) { return 803; }
            if !reads_invalid(&document, 22) { return 804; }
            if !reads_invalid(&document, highest) { return 805; }
            if !unchanged(&document, &original, 5) { return 806; }
        },
    }
    let short = ascii_corpus();
    let short_original = [byte(0), byte(65), byte(127)];
    let short_converted = owned(&short);
    match short_converted {
        FromBytes::Invalid(unexpected_short_owner) => { return 807; },
        FromBytes::Ok(short_document) => {
            if !reads_invalid(&short_document, 4) { return 808; }
            if !reads_invalid(&short_document, highest) { return 809; }
            if !unchanged(&short_document, &short_original, 3) { return 810; }
        },
    }
    let empty = new_text();
    if !reads_invalid(&empty, 1) { return 811; }
    if !reads_invalid(&empty, highest) { return 812; }
    if byte_len(&empty) != 0 { return 813; }
    return 0;
}

fn test_empty_and_end_repeat_without_change() -> i32 {
    let empty = new_text();
    if !reads_end(&empty, 0) { return 901; }
    if !reads_end(&empty, 0) { return 902; }
    if !reads_end(&empty, 0) { return 903; }
    if byte_len(&empty) != 0 || scalar_count(&empty) != 0 { return 904; }
    let none: [u8; 0] = [];
    let converted_empty = owned(&none);
    match converted_empty {
        FromBytes::Invalid(unexpected_empty_owner) => { return 905; },
        FromBytes::Ok(empty_document) => {
            if !reads_end(&empty_document, 0) { return 906; }
            if !reads_end(&empty_document, 0) { return 907; }
            if !unchanged(&empty_document, &none, 0) { return 908; }
        },
    }
    let corpus = mixed_corpus();
    let original = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 909; },
        FromBytes::Ok(document) => {
            if !reads_end(&document, 11) { return 910; }
            if !reads_end(&document, 11) { return 911; }
            if !reads_end(&document, 11) { return 912; }
            if !unchanged(&document, &original, 5) { return 913; }
        },
    }
    return 0;
}

fn test_repeated_read_leaves_cursor_and_text() -> i32 {
    let corpus = mixed_corpus();
    let original = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1001; },
        FromBytes::Ok(document) => {
            let cursor = 6;
            if !reads_value(&document, cursor, 128512, 10) { return 1002; }
            if !reads_value(&document, cursor, 128512, 10) { return 1003; }
            if cursor != 6 { return 1004; }
            let interior = 7;
            if !reads_invalid(&document, interior) { return 1005; }
            if !reads_invalid(&document, interior) { return 1006; }
            if interior != 7 { return 1007; }
            if !unchanged(&document, &original, 5) { return 1008; }
        },
    }
    return 0;
}

fn test_caller_cursor_walks_whole_text() -> i32 {
    let corpus = mixed_corpus();
    let original = [byte(65), byte(194), byte(162), byte(226), byte(130), byte(172), byte(240), byte(159), byte(152), byte(128), byte(122)];
    let scalars = [65, 162, 8364, 128512, 122];
    let nexts = [1, 3, 6, 10, 11];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1101; },
        FromBytes::Ok(document) => {
            let mut cursor = 0;
            let mut index = 0;
            let mut walking = true;
            while walking {
                let read = next_scalar(&document, cursor);
                match read {
                    ScalarRead::Value(step) => {
                        if index >= 5 { return 1102; }
                        let next = step.next_byte_offset;
                        if !step_is(step, scalars[index], nexts[index]) { return 1103; }
                        cursor = next;
                        index = index + 1;
                    },
                    ScalarRead::End => { walking = false; },
                    ScalarRead::InvalidOffset => { return 1104; },
                }
            }
            if index != 5 || cursor != 11 { return 1105; }
            if !unchanged(&document, &original, 5) { return 1106; }
        },
    }
    return 0;
}

// The token scan is split into helpers so that no single function exceeds the
// native per-function slot limit. Each helper owns only the hand-written
// arrays it compares against; the fixed values and return codes are unchanged.
fn scan_token_bounds(document: &Text) -> i32 {
    let scalars = [120, 49, 32, 99, 97, 102, 101, 769, 32, 8364, 53, 32, 128512, 33];
    let nexts = [1, 2, 3, 4, 5, 6, 7, 9, 10, 13, 14, 15, 19, 20];
    let mut bounds = [0, 0, 0, 0, 0, 0, 0, 0];
    let mut cursor = 0;
    let mut token_start = 0;
    let mut index = 0;
    let mut tokens = 0;
    let mut scanning = true;
    while scanning {
        let read = next_scalar(&*document, cursor);
        match read {
            ScalarRead::Value(step) => {
                if index >= 14 { return 1202; }
                let scalar = step.scalar;
                let next = step.next_byte_offset;
                if scalar != scalars[index] || next != nexts[index] { return 1203; }
                if scalar == 32 {
                    if tokens >= 3 { return 1204; }
                    bounds[tokens * 2] = token_start;
                    bounds[tokens * 2 + 1] = cursor;
                    tokens = tokens + 1;
                    token_start = next;
                }
                cursor = next;
                index = index + 1;
            },
            ScalarRead::End => {
                if tokens != 3 { return 1205; }
                bounds[6] = token_start;
                bounds[7] = cursor;
                tokens = tokens + 1;
                scanning = false;
            },
            ScalarRead::InvalidOffset => { return 1206; },
        }
    }
    if index != 14 || cursor != 20 || tokens != 4 { return 1207; }
    let matched = token_bounds_match(&bounds);
    if matched != 0 { return matched; }
    return token_spellings_recovered(&*document, &bounds);
}

fn token_bounds_match(bounds: &[i32]) -> i32 {
    let expected_bounds = [0, 2, 3, 9, 10, 14, 15, 20];
    let mut position = 0;
    while position < 8 {
        if bounds[position] != expected_bounds[position] { return 1208; }
        position = position + 1;
    }
    return 0;
}

fn token_spellings_recovered(document: &Text, bounds: &[i32]) -> i32 {
    let first = [byte(120), byte(49)];
    let second = [byte(99), byte(97), byte(102), byte(101), byte(204), byte(129)];
    let third = [byte(226), byte(130), byte(172), byte(53)];
    let fourth = [byte(240), byte(159), byte(152), byte(128), byte(33)];
    if !sliced_exactly(&*document, bounds[0], bounds[1], &first) { return 1209; }
    if !sliced_exactly(&*document, bounds[2], bounds[3], &second) { return 1210; }
    if !sliced_exactly(&*document, bounds[4], bounds[5], &third) { return 1211; }
    if !sliced_exactly(&*document, bounds[6], bounds[7], &fourth) { return 1212; }
    return 0;
}

fn test_scan_mixed_tokens_recovers_spelling() -> i32 {
    let corpus = token_corpus();
    let original = [byte(120), byte(49), byte(32), byte(99), byte(97), byte(102), byte(101), byte(204), byte(129), byte(32), byte(226), byte(130), byte(172), byte(53), byte(32), byte(240), byte(159), byte(152), byte(128), byte(33)];
    let converted = owned(&corpus);
    match converted {
        FromBytes::Invalid(unexpected_owner) => { return 1201; },
        FromBytes::Ok(document) => {
            let scanned = scan_token_bounds(&document);
            if scanned != 0 { return scanned; }
            if !unchanged(&document, &original, 14) { return 1213; }
        },
    }
    return 0;
}

fn main() -> i32 {
    let ascii = test_nul_and_ascii_scalars();
    if ascii != 0 { return ascii; }
    let two = test_two_byte_boundaries();
    if two != 0 { return two; }
    let three = test_three_byte_boundaries_around_surrogates();
    if three != 0 { return three; }
    let four = test_four_byte_boundaries_to_max_scalar();
    if four != 0 { return four; }
    let special = test_bom_combining_and_noncharacters();
    if special != 0 { return special; }
    let interior = test_every_interior_offset_rejected();
    if interior != 0 { return interior; }
    let negative = test_negative_offsets_rejected();
    if negative != 0 { return negative; }
    let past_end = test_offsets_past_end_rejected();
    if past_end != 0 { return past_end; }
    let end = test_empty_and_end_repeat_without_change();
    if end != 0 { return end; }
    let repeated = test_repeated_read_leaves_cursor_and_text();
    if repeated != 0 { return repeated; }
    let walk = test_caller_cursor_walks_whole_text();
    if walk != 0 { return walk; }
    let tokens = test_scan_mixed_tokens_recovers_spelling();
    if tokens != 0 { return tokens; }
    return 42;
}
