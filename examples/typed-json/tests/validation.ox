use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use crate::json::Validation;
use crate::json::validate;
use crate::json::StringResult;
use crate::json::quote;
use crate::json::decode_string;
use crate::text::equals_utf8;

fn valid(input: &[u8]) -> bool {
    let result = validate(&*input);
    match result {
        Validation::Valid => { return true; },
        Validation::Syntax(offset) => { return false; },
        Validation::InvalidUtf8(offset) => { return false; },
        Validation::CapacityOverflow => { return false; },
        Validation::AllocationFailed => { return false; },
        Validation::MemoryLimit => { return false; },
    }
}

fn syntax(input: &[u8], expected: i32) -> bool {
    let result = validate(&*input);
    match result {
        Validation::Valid => { return false; },
        Validation::Syntax(offset) => { return offset == expected; },
        Validation::InvalidUtf8(offset) => { return false; },
        Validation::CapacityOverflow => { return false; },
        Validation::AllocationFailed => { return false; },
        Validation::MemoryLimit => { return false; },
    }
}

fn accepted() -> i32 {
    let object = b" \n{\"a\":[{},[],null,true,false,{\"a\":-0.12e+100}],\"a\":25E-2}\r\t";
    if !valid(&object) { return 1; }
    let unicode = b"\"東京🦀\\u0000\\uD83E\\uDD80\\b\\f\\n\\r\\t\\\\\\/\\\"\"";
    if !valid(&unicode) { return 2; }
    let exponent = b"6.022e+99999";
    if !valid(&exponent) { return 3; }
    let scalar = b"0";
    if !valid(&scalar) { return 4; }
    return 0;
}

fn rejected() -> i32 {
    let empty = b"";
    let trailing = b"null false";
    let array_comma = b"[1,]";
    let object_comma = b"{\"a\":1,}";
    let no_colon = b"{\"a\" 1}";
    let missing_value = b"{\"a\":}";
    let leading_zero = b"-01";
    let empty_exp = b"1e+";
    let empty_frac = b"1.";
    let unclosed = b"\"hi";
    let space = b"\x0cnull";
    let control = b"\"\x00\"";
    let bad_escape = b"\"\\x20\"";
    let unmatched_high = b"\"\\uD800x\"";
    let unmatched_low = b"\"\\uDD80\"";
    let wrong_pair = b"\"\\uD800\\u0020\"";
    let bad_hex = b"\"\\u000G\"";
    if !syntax(&empty, 0) || !syntax(&trailing, 5) { return 10; }
    if !syntax(&array_comma, 3) || !syntax(&object_comma, 7) { return 11; }
    if !syntax(&no_colon, 5) || !syntax(&missing_value, 5) { return 12; }
    if !syntax(&leading_zero, 2) || !syntax(&empty_exp, 3) || !syntax(&empty_frac, 2) { return 13; }
    if !syntax(&unclosed, 3) || !syntax(&space, 0) || !syntax(&control, 1) { return 14; }
    if !syntax(&bad_escape, 2) || !syntax(&unmatched_high, 7) { return 15; }
    if !syntax(&unmatched_low, 3) || !syntax(&wrong_pair, 9) || !syntax(&bad_hex, 6) { return 16; }
    let bad_encoding = b"\"\xff\"";
    let result = validate(&bad_encoding);
    match result {
        Validation::InvalidUtf8(offset) => { if offset == 1 { return 0; } return 17; },
        Validation::Valid => { return 18; },
        Validation::Syntax(offset) => { return 19; },
        Validation::CapacityOverflow => { return 20; },
        Validation::AllocationFailed => { return 21; },
        Validation::MemoryLimit => { return 22; },
    }
}

fn resize_ok(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn finish_document(bytes: &mut [u8]) -> () {
    bytes[256] = 48u8;
    for index in 257..513 { bytes[index] = 93u8; }
    return;
}

fn deep_document() -> i32 {
    let mut document = new();
    if !resize_ok(try_resize(&mut document, 513, 91u8)) { return 30; }
    finish_document(&mut document);
    if !valid(&document) { return 31; }
    let mut pressure = new();
    if !resize_ok(try_resize(&mut pressure, 2048, 0u8)) { return 32; }
    let limited = validate(&document);
    if len(&pressure) != 2048 { return 33; }
    match limited {
        Validation::MemoryLimit => { return 0; },
        Validation::Valid => { return 34; },
        Validation::Syntax(offset) => { return 35; },
        Validation::InvalidUtf8(offset) => { return 36; },
        Validation::CapacityOverflow => { return 37; },
        Validation::AllocationFailed => { return 38; },
    }
}

fn string_codec() -> i32 {
    let original = b"東京🦀/\"\\\x00\x08\x0c\r\n\t";
    let quoted = b"\"東京🦀/\\\"\\\\\\u0000\\u0008\\u000c\\u000d\\u000a\\u0009\"";
    let result = quote(&original);
    match result {
        StringResult::Value(encoded) => { if !equals_utf8(&encoded, &quoted) { return 40; } },
        StringResult::Error(error) => { return 41; },
    }
    let input = b"  \"東京\\uD83E\\uDD80\\/\\\"\\\\\\u0000\\b\\f\\r\\n\\t\" \r\n";
    let decoded = decode_string(&input);
    match decoded {
        StringResult::Value(value) => { if !equals_utf8(&value, &original) { return 42; } },
        StringResult::Error(error) => { return 43; },
    }
    let trailing = b"\"a\"0";
    let invalid = decode_string(&trailing);
    match invalid {
        StringResult::Value(value) => { return 44; },
        StringResult::Error(error) => {
            match error {
                Validation::Syntax(offset) => { if offset != 3 { return 45; } },
                Validation::Valid => { return 46; },
                Validation::InvalidUtf8(offset) => { return 46; },
                Validation::CapacityOverflow => { return 46; },
                Validation::AllocationFailed => { return 46; },
                Validation::MemoryLimit => { return 46; },
            }
        },
    }
    let bad_encoding = b"a\xff";
    let refused = quote(&bad_encoding);
    match refused {
        StringResult::Value(value) => { return 47; },
        StringResult::Error(error) => {
            match error {
                Validation::InvalidUtf8(offset) => { if offset == 1 { return 0; } return 48; },
                Validation::Valid => { return 49; },
                Validation::Syntax(offset) => { return 49; },
                Validation::CapacityOverflow => { return 49; },
                Validation::AllocationFailed => { return 49; },
                Validation::MemoryLimit => { return 49; },
            }
        },
    }
}

fn string_limit() -> i32 {
    let mut data = new();
    if !resize_ok(try_resize(&mut data, 768, 0u8)) { return 50; }
    let limited = quote(&data);
    match limited {
        StringResult::Value(value) => { return 51; },
        StringResult::Error(error) => {
            match error {
                Validation::MemoryLimit => {},
                Validation::Valid => { return 52; },
                Validation::Syntax(offset) => { return 52; },
                Validation::InvalidUtf8(offset) => { return 52; },
                Validation::CapacityOverflow => { return 52; },
                Validation::AllocationFailed => { return 52; },
            }
        },
    }
    if len(&data) != 768 { return 53; }
    let example = b"雪";
    let expected = b"\"雪\"";
    let retried = quote(&example);
    match retried {
        StringResult::Value(value) => { if equals_utf8(&value, &expected) { return 0; } return 54; },
        StringResult::Error(error) => { return 55; },
    }
}

fn main() -> i32 {
    let passed = accepted();
    if passed != 0 { return passed; }
    let invalid = rejected();
    if invalid != 0 { return invalid; }
    let deep = deep_document();
    if deep != 0 { return deep; }
    let codec = string_codec();
    if codec != 0 { return codec; }
    return string_limit();
}
