use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use hex::EncodeResult;
use hex::DecodeResult;
use hex::encode_lower;
use hex::decode;

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
    if actual.len() != expected.len() { return false; }
    let mut offset = 0;
    while offset < expected.len() {
        if actual[offset] != expected[offset] { return false; }
        offset = offset + 1;
    }
    return true;
}

// Nonuniform destination sentinel for one buffer position.
fn sentinel(seed: i32, offset: i32) -> i32 {
    return (seed + offset * 37) % 256;
}

fn fill_sentinels(bytes: &mut [u8], seed: i32) -> bool {
    let mut offset = 0;
    while offset < bytes.len() {
        bytes[offset] = byte(sentinel(seed, offset));
        offset = offset + 1;
    }
    return true;
}

// Every byte from `start` onward still holds its original sentinel.
fn sentinels_from(bytes: &[u8], start: i32, seed: i32) -> bool {
    let mut offset = start;
    while offset < bytes.len() {
        let current = bytes[offset];
        if current.to_i32() != sentinel(seed, offset) { return false; }
        offset = offset + 1;
    }
    return true;
}

// The written prefix equals `expected`; the unwritten suffix keeps its sentinels.
fn prefix_then_sentinels(bytes: &[u8], expected: &[u8], seed: i32) -> bool {
    if expected.len() > bytes.len() { return false; }
    let mut offset = 0;
    while offset < expected.len() {
        if bytes[offset] != expected[offset] { return false; }
        offset = offset + 1;
    }
    return sentinels_from(&*bytes, expected.len(), seed);
}

fn fill_identity(bytes: &mut [u8]) -> bool {
    let mut offset = 0;
    while offset < bytes.len() {
        bytes[offset] = byte(offset);
        offset = offset + 1;
    }
    return true;
}

// Exactly 256 bytes holding 0, 1, ..., 255 in order.
fn is_identity(bytes: &[u8]) -> bool {
    if bytes.len() != 256 { return false; }
    let mut offset = 0;
    while offset < 256 {
        let current = bytes[offset];
        if current.to_i32() != offset { return false; }
        offset = offset + 1;
    }
    return true;
}

fn encode_written(result: EncodeResult, expected: i32) -> bool {
    match result {
        EncodeResult::Written(count) => { return count == expected; },
        EncodeResult::BufferTooSmall(required) => { return false; },
        EncodeResult::LengthOverflow => { return false; },
    }
}

fn encode_too_small(result: EncodeResult, expected: i32) -> bool {
    match result {
        EncodeResult::Written(count) => { return false; },
        EncodeResult::BufferTooSmall(required) => { return required == expected; },
        EncodeResult::LengthOverflow => { return false; },
    }
}

fn decode_written(result: DecodeResult, expected: i32) -> bool {
    match result {
        DecodeResult::Written(count) => { return count == expected; },
        DecodeResult::OddLength => { return false; },
        DecodeResult::InvalidDigit(offset) => { return false; },
        DecodeResult::BufferTooSmall(required) => { return false; },
    }
}

fn decode_odd(result: DecodeResult) -> bool {
    match result {
        DecodeResult::Written(count) => { return false; },
        DecodeResult::OddLength => { return true; },
        DecodeResult::InvalidDigit(offset) => { return false; },
        DecodeResult::BufferTooSmall(required) => { return false; },
    }
}

fn decode_invalid_at(result: DecodeResult, expected: i32) -> bool {
    match result {
        DecodeResult::Written(count) => { return false; },
        DecodeResult::OddLength => { return false; },
        DecodeResult::InvalidDigit(offset) => { return offset == expected; },
        DecodeResult::BufferTooSmall(required) => { return false; },
    }
}

fn decode_too_small(result: DecodeResult, expected: i32) -> bool {
    match result {
        DecodeResult::Written(count) => { return false; },
        DecodeResult::OddLength => { return false; },
        DecodeResult::InvalidDigit(offset) => { return false; },
        DecodeResult::BufferTooSmall(required) => { return required == expected; },
    }
}

// Encoding into `capacity` sentinel bytes writes exactly `expected` and keeps the suffix.
fn encodes_into(input: &[u8], capacity: i32, seed: i32, expected: &[u8]) -> bool {
    let mut buffer = new();
    let ready = try_resize(&mut buffer, capacity, byte(0));
    if !grew(ready) { return false; }
    if !fill_sentinels(&mut buffer, seed) { return false; }
    let result = encode_lower(&*input, &mut buffer);
    if !encode_written(result, expected.len()) { return false; }
    return prefix_then_sentinels(&buffer, &*expected, seed);
}

// A short destination reports `required` and keeps every sentinel byte.
fn encode_refused(input: &[u8], capacity: i32, seed: i32, required: i32) -> bool {
    let mut buffer = new();
    let ready = try_resize(&mut buffer, capacity, byte(0));
    if !grew(ready) { return false; }
    if !fill_sentinels(&mut buffer, seed) { return false; }
    let result = encode_lower(&*input, &mut buffer);
    if !encode_too_small(result, required) { return false; }
    return sentinels_from(&buffer, 0, seed);
}

fn decodes_into(input: &[u8], capacity: i32, seed: i32, expected: &[u8]) -> bool {
    let mut buffer = new();
    let ready = try_resize(&mut buffer, capacity, byte(0));
    if !grew(ready) { return false; }
    if !fill_sentinels(&mut buffer, seed) { return false; }
    let result = decode(&*input, &mut buffer);
    if !decode_written(result, expected.len()) { return false; }
    return prefix_then_sentinels(&buffer, &*expected, seed);
}

fn odd_refused(input: &[u8], capacity: i32, seed: i32) -> bool {
    let mut buffer = new();
    let ready = try_resize(&mut buffer, capacity, byte(0));
    if !grew(ready) { return false; }
    if !fill_sentinels(&mut buffer, seed) { return false; }
    let result = decode(&*input, &mut buffer);
    if !decode_odd(result) { return false; }
    return sentinels_from(&buffer, 0, seed);
}

fn invalid_refused(input: &[u8], capacity: i32, seed: i32, expected: i32) -> bool {
    let mut buffer = new();
    let ready = try_resize(&mut buffer, capacity, byte(0));
    if !grew(ready) { return false; }
    if !fill_sentinels(&mut buffer, seed) { return false; }
    let result = decode(&*input, &mut buffer);
    if !decode_invalid_at(result, expected) { return false; }
    return sentinels_from(&buffer, 0, seed);
}

fn short_refused(input: &[u8], capacity: i32, seed: i32, required: i32) -> bool {
    let mut buffer = new();
    let ready = try_resize(&mut buffer, capacity, byte(0));
    if !grew(ready) { return false; }
    if !fill_sentinels(&mut buffer, seed) { return false; }
    let result = decode(&*input, &mut buffer);
    if !decode_too_small(result, required) { return false; }
    return sentinels_from(&buffer, 0, seed);
}

// `value` placed after a valid `0` is rejected at offset 1, and before one at offset 0.
fn rejected_both_nibbles(value: i32) -> bool {
    let low = [byte(48), byte(value)];
    let high = [byte(value), byte(48)];
    if !invalid_refused(&low, 1, value, 1) { return false; }
    return invalid_refused(&high, 1, value + 1, 0);
}

// Fixed digit tables, independent of the package: index = nibble value.
fn nibble_of(value: i32) -> i32 {
    let lower = b"0123456789abcdef";
    let upper = b"0123456789ABCDEF";
    let mut nibble = 0;
    while nibble < 16 {
        let small = lower[nibble];
        let large = upper[nibble];
        if small.to_i32() == value || large.to_i32() == value { return nibble; }
        nibble = nibble + 1;
    }
    return -1;
}

fn test_encode_all_bytes_fixed_oracle() -> i32 {
    let lower = b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff";
    let mut input = new();
    let input_ready = try_resize(&mut input, 256, byte(0));
    if !grew(input_ready) { return 101; }
    if !fill_identity(&mut input) { return 102; }
    if !encodes_into(&input, 515, 11, &lower) { return 103; }
    if !encodes_into(&input, 512, 13, &lower) { return 104; }
    if !is_identity(&input) { return 105; }
    return 0;
}

fn test_decode_all_bytes_every_case() -> i32 {
    let lower = b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff";
    let upper = b"000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F202122232425262728292A2B2C2D2E2F303132333435363738393A3B3C3D3E3F404142434445464748494A4B4C4D4E4F505152535455565758595A5B5C5D5E5F606162636465666768696A6B6C6D6E6F707172737475767778797A7B7C7D7E7F808182838485868788898A8B8C8D8E8F909192939495969798999A9B9C9D9E9FA0A1A2A3A4A5A6A7A8A9AAABACADAEAFB0B1B2B3B4B5B6B7B8B9BABBBCBDBEBFC0C1C2C3C4C5C6C7C8C9CACBCCCDCECFD0D1D2D3D4D5D6D7D8D9DADBDCDDDEDFE0E1E2E3E4E5E6E7E8E9EAEBECEDEEEFF0F1F2F3F4F5F6F7F8F9FAFBFCFDFEFF";
    let mixed = b"000102030405060708090a0b0C0d0e0F101112131415161718191a1B1c1d1E1f202122232425262728292A2b2c2D2e2f303132333435363738393a3b3C3d3e3F404142434445464748494a4B4c4d4E4f505152535455565758595A5b5c5D5e5f606162636465666768696a6b6C6d6e6F707172737475767778797a7B7c7d7E7f808182838485868788898A8b8c8D8e8f909192939495969798999a9b9C9d9e9FA0a1A2a3A4a5A6a7A8a9AaaBAcadAEafB0b1B2b3B4b5B6b7B8b9BAbbBcbDBebfC0c1C2c3C4c5C6c7C8c9CacbCCcdCecFD0d1D2d3D4d5D6d7D8d9DadBDcddDEdfE0e1E2e3E4e5E6e7E8e9EAebEceDEeefF0f1F2f3F4f5F6f7F8f9FafbFCfdFefF";
    let mut identity = new();
    let identity_ready = try_resize(&mut identity, 256, byte(0));
    if !grew(identity_ready) { return 201; }
    if !fill_identity(&mut identity) { return 202; }
    if !decodes_into(&lower, 259, 17, &identity) { return 203; }
    if !decodes_into(&upper, 256, 19, &identity) { return 204; }
    if !decodes_into(&mixed, 300, 23, &identity) { return 205; }
    if !is_identity(&identity) { return 206; }
    return 0;
}

fn test_encode_empty_input() -> i32 {
    let empty: [u8; 0] = [];
    let mut none: [u8; 0] = [];
    let into_none = encode_lower(&empty, &mut none);
    if !encode_written(into_none, 0) { return 301; }
    if !encodes_into(&empty, 4, 29, &empty) { return 302; }
    if none.len() != 0 || empty.len() != 0 { return 303; }
    return 0;
}

fn test_encode_destination_sizes() -> i32 {
    let input = b"\x00\x80\xff";
    let expected = b"0080ff";
    let single = b"\x7f";
    let single_expected = b"7f";
    if !encode_refused(&input, 0, 31, 6) { return 401; }
    if !encode_refused(&input, 5, 37, 6) { return 402; }
    if !encode_refused(&input, 1, 41, 6) { return 403; }
    if !encodes_into(&input, 6, 43, &expected) { return 404; }
    if !encodes_into(&input, 9, 47, &expected) { return 405; }
    if !encode_refused(&single, 1, 53, 2) { return 406; }
    if !encodes_into(&single, 2, 59, &single_expected) { return 407; }
    if !encodes_into(&single, 64, 61, &single_expected) { return 408; }
    return 0;
}

fn test_encode_nibble_order_lowercase() -> i32 {
    let input = b"\x0f\xf0\x1a\xa1\x09\x90\x7f\x80\xc3\x3c\xde\xad";
    let expected = b"0ff01aa109907f80c33cdead";
    if !encodes_into(&input, 24, 67, &expected) { return 501; }
    if !encodes_into(&input, 25, 71, &expected) { return 502; }
    return 0;
}

fn test_decode_empty_input() -> i32 {
    let empty: [u8; 0] = [];
    let mut none: [u8; 0] = [];
    let into_none = decode(&empty, &mut none);
    if !decode_written(into_none, 0) { return 601; }
    if !decodes_into(&empty, 3, 73, &empty) { return 602; }
    if none.len() != 0 || empty.len() != 0 { return 603; }
    return 0;
}

fn test_decode_destination_sizes() -> i32 {
    let input = b"00Ff80";
    let expected = b"\x00\xff\x80";
    if !short_refused(&input, 0, 79, 3) { return 701; }
    if !short_refused(&input, 2, 83, 3) { return 702; }
    if !decodes_into(&input, 3, 89, &expected) { return 703; }
    if !decodes_into(&input, 6, 97, &expected) { return 704; }
    return 0;
}

fn test_decode_odd_length_first() -> i32 {
    let g = b"g";
    let digit = b"0";
    let middle = b"0g0";
    let valid_odd = b"abc";
    let long_odd = b"00112233445";
    let high_odd = b"\xff";
    let invalid_odd = b"zz0";
    if !odd_refused(&g, 0, 101) { return 801; }
    if !odd_refused(&g, 4, 103) { return 802; }
    if !odd_refused(&digit, 8, 107) { return 803; }
    if !odd_refused(&middle, 0, 109) { return 804; }
    if !odd_refused(&middle, 4, 113) { return 805; }
    if !odd_refused(&valid_odd, 0, 127) { return 806; }
    if !odd_refused(&valid_odd, 2, 131) { return 807; }
    if !odd_refused(&long_odd, 0, 137) { return 808; }
    if !odd_refused(&high_odd, 1, 139) { return 809; }
    if !odd_refused(&invalid_odd, 9, 149) { return 810; }
    return 0;
}

fn test_decode_invalid_each_nibble_position() -> i32 {
    let first_high = b"g0";
    let first_low = b"0g";
    let second_high = b"00G0";
    let second_low = b"000G";
    let third_high = b"0a1b:c";
    let third_low = b"0a1bc/";
    if !invalid_refused(&first_high, 1, 151, 0) { return 901; }
    if !invalid_refused(&first_low, 1, 157, 1) { return 902; }
    if !invalid_refused(&second_high, 2, 163, 2) { return 903; }
    if !invalid_refused(&second_low, 2, 167, 3) { return 904; }
    if !invalid_refused(&third_high, 3, 173, 4) { return 905; }
    if !invalid_refused(&third_low, 3, 179, 5) { return 906; }
    return 0;
}

fn test_decode_invalid_first_middle_last() -> i32 {
    let first = b"z0112233445566";
    let middle = b"0011223z445566";
    let last = b"001122334455667z";
    let two = b"00x1y2";
    let spaces = b"0011  zz";
    let spaced = b"00ff 0";
    let prefixed = b"0x";
    if !invalid_refused(&first, 7, 181, 0) { return 1001; }
    if !invalid_refused(&middle, 7, 191, 7) { return 1002; }
    if !invalid_refused(&last, 8, 193, 15) { return 1003; }
    if !invalid_refused(&two, 3, 197, 2) { return 1004; }
    if !invalid_refused(&spaces, 4, 199, 4) { return 1005; }
    if !invalid_refused(&spaced, 3, 211, 4) { return 1006; }
    if !invalid_refused(&prefixed, 1, 223, 1) { return 1007; }
    return 0;
}

fn test_decode_rejects_byte_classes() -> i32 {
    if !rejected_both_nibbles(0) { return 1101; }
    if !rejected_both_nibbles(9) { return 1102; }
    if !rejected_both_nibbles(10) { return 1103; }
    if !rejected_both_nibbles(13) { return 1104; }
    if !rejected_both_nibbles(32) { return 1105; }
    if !rejected_both_nibbles(43) { return 1106; }
    if !rejected_both_nibbles(44) { return 1107; }
    if !rejected_both_nibbles(45) { return 1108; }
    if !rejected_both_nibbles(46) { return 1109; }
    if !rejected_both_nibbles(47) { return 1110; }
    if !rejected_both_nibbles(58) { return 1111; }
    if !rejected_both_nibbles(64) { return 1112; }
    if !rejected_both_nibbles(71) { return 1113; }
    if !rejected_both_nibbles(95) { return 1114; }
    if !rejected_both_nibbles(96) { return 1115; }
    if !rejected_both_nibbles(103) { return 1116; }
    if !rejected_both_nibbles(120) { return 1117; }
    if !rejected_both_nibbles(128) { return 1118; }
    if !rejected_both_nibbles(176) { return 1119; }
    if !rejected_both_nibbles(185) { return 1120; }
    if !rejected_both_nibbles(193) { return 1121; }
    if !rejected_both_nibbles(230) { return 1122; }
    if !rejected_both_nibbles(255) { return 1123; }
    return 0;
}

fn test_decode_classifies_every_byte() -> i32 {
    let mut value = 0;
    let mut accepted = 0;
    while value < 256 {
        let pair = [byte(48), byte(value)];
        let nibble = nibble_of(value);
        if nibble < 0 {
            if !invalid_refused(&pair, 1, value, 1) { return 1201; }
        }
        if nibble >= 0 {
            let expected = [byte(nibble)];
            if !decodes_into(&pair, 2, value, &expected) { return 1202; }
            accepted = accepted + 1;
        }
        value = value + 1;
    }
    if accepted != 22 { return 1203; }
    return 0;
}

fn test_decode_invalid_beats_short_destination() -> i32 {
    let both = b"00zz";
    let late = b"0011223344g5";
    let long_late = b"aabbccddeeffAABBCCDDEEFF0011223344556677889g";
    if !invalid_refused(&both, 0, 227, 2) { return 1301; }
    if !invalid_refused(&both, 1, 229, 2) { return 1302; }
    if !invalid_refused(&late, 0, 233, 10) { return 1303; }
    if !invalid_refused(&late, 2, 239, 10) { return 1304; }
    if !invalid_refused(&late, 6, 241, 10) { return 1305; }
    if !invalid_refused(&late, 9, 251, 10) { return 1306; }
    if !invalid_refused(&long_late, 22, 3, 43) { return 1307; }
    if !invalid_refused(&long_late, 5, 5, 43) { return 1308; }
    return 0;
}

fn test_decode_too_small_only_for_valid_even() -> i32 {
    let eight = b"0011223344556677";
    let one = b"Ff";
    let expected = b"\x00\x11\x22\x33\x44\x55\x66\x77";
    if !short_refused(&eight, 0, 7, 8) { return 1401; }
    if !short_refused(&eight, 7, 9, 8) { return 1402; }
    if !short_refused(&one, 0, 15, 1) { return 1403; }
    if !decodes_into(&eight, 8, 21, &expected) { return 1404; }
    return 0;
}

fn test_inputs_unchanged() -> i32 {
    let encode_input = b"\x00\x80\xff\x10";
    let encode_copy = b"\x00\x80\xff\x10";
    let encoded = b"0080ff10";
    let decode_input = b"00Ff80";
    let decode_copy = b"00Ff80";
    let decoded = b"\x00\xff\x80";
    let invalid_input = b"0011zz";
    let invalid_copy = b"0011zz";
    let odd_input = b"abc";
    let odd_copy = b"abc";
    if !encodes_into(&encode_input, 8, 25, &encoded) { return 1501; }
    if !encode_refused(&encode_input, 7, 27, 8) { return 1502; }
    if !same_bytes(&encode_input, &encode_copy) { return 1503; }
    if !decodes_into(&decode_input, 3, 33, &decoded) { return 1504; }
    if !short_refused(&decode_input, 1, 35, 3) { return 1505; }
    if !same_bytes(&decode_input, &decode_copy) { return 1506; }
    if !invalid_refused(&invalid_input, 3, 39, 4) { return 1507; }
    if !same_bytes(&invalid_input, &invalid_copy) { return 1508; }
    if !odd_refused(&odd_input, 2, 45) { return 1509; }
    if !same_bytes(&odd_input, &odd_copy) { return 1510; }
    return 0;
}

fn test_round_trip_supplement() -> i32 {
    let input = b"\x00\x7f\x80\xff\x5a\xa5";
    let mut encoded = new();
    let encoded_ready = try_resize(&mut encoded, 12, byte(0));
    if !grew(encoded_ready) { return 1601; }
    let encoding = encode_lower(&input, &mut encoded);
    if !encode_written(encoding, 12) { return 1602; }
    let mut decoded = new();
    let decoded_ready = try_resize(&mut decoded, 6, byte(0));
    if !grew(decoded_ready) { return 1603; }
    let decoding = decode(&encoded, &mut decoded);
    if !decode_written(decoding, 6) { return 1604; }
    if !same_bytes(&decoded, &input) { return 1605; }
    return 0;
}

fn main() -> i32 {
    let inputs_unchanged = test_inputs_unchanged();
    if inputs_unchanged != 0 { return inputs_unchanged; }
    return 42;
}
