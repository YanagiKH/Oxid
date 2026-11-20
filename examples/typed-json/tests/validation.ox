use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use crate::json::Validation;
use crate::json::validate;

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

fn main() -> i32 {
    let passed = accepted();
    if passed != 0 { return passed; }
    let invalid = rejected();
    if invalid != 0 { return invalid; }
    return deep_document();
}
