use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use alloc::bytes::truncate;
use crate::json::Validation;
use crate::json::validate;
use crate::json::StringResult;
use crate::json::quote;
use crate::json::decode_string;
use crate::text::equals_utf8;
use crate::json::Document;
use crate::json::Kind;
use crate::json::kind;
use crate::json::ParseResult;
use crate::json::ParseFailure;
use crate::json::CopyResult;
use crate::json::parse;
use crate::json::node;
use crate::json::node_count;
use crate::json::first_child;
use crate::json::next_sibling;
use crate::json::append_key;
use crate::json::append_value;
use crate::types::option::Option;
use crate::types::option::unwrap_or;

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

fn same(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() { return false; }
    for index in 0..left.len() {
        if left[index] != right[index] { return false; }
    }
    return true;
}

fn copied(result: CopyResult) -> bool {
    match result {
        CopyResult::Copied => { return true; },
        CopyResult::Missing => { return false; },
        CopyResult::Error(error) => { return false; },
    }
}

fn check_values(document: &Document) -> i32 {
    let mut output = new();
    if !copied(append_key(&*document, 1, &mut output)) { return 60; }
    if !copied(append_value(&*document, 4, &mut output)) { return 61; }
    let expected = b"\"list\"\"雪\"";
    if !same(&output, &expected) { return 62; }
    truncate(&mut output, 0);
    if !copied(append_value(&*document, 1, &mut output)) { return 63; }
    let array = b"[1, {\"x\":\"雪\"}, []]";
    if !same(&output, &array) { return 64; }
    let result = append_key(&*document, 2, &mut output);
    match result {
        CopyResult::Missing => { if same(&output, &array) { return 0; } return 65; },
        CopyResult::Copied => { return 66; },
        CopyResult::Error(error) => { return 67; },
    }
}

fn check_tree(document: &Document) -> i32 {
    if node_count(&*document) != 7 { return 70; }
    if unwrap_or(first_child(&*document, 0), -1) != 1 { return 71; }
    if unwrap_or(first_child(&*document, 1), -1) != 2 { return 72; }
    if unwrap_or(next_sibling(&*document, 2), -1) != 3 { return 73; }
    if unwrap_or(first_child(&*document, 3), -1) != 4 { return 74; }
    if unwrap_or(next_sibling(&*document, 3), -1) != 5 { return 75; }
    if unwrap_or(next_sibling(&*document, 1), -1) != 6 { return 76; }
    if unwrap_or(next_sibling(&*document, 4), -1) != -1 { return 77; }
    if unwrap_or(first_child(&*document, 5), -1) != -1 { return 78; }
    if unwrap_or(next_sibling(&*document, 7), -1) != -1 { return 79; }
    return 0;
}

fn parsed_values() -> i32 {
    let input = b" \n{\"list\": [1, {\"x\":\"雪\"}, []], \"other\":true} ";
    let mut owned = new();
    if !resize_ok(try_extend(&mut owned, &input)) { return 80; }
    let result = parse(owned);
    match result {
        ParseResult::Error(failure) => { return 81; },
        ParseResult::Parsed(document) => {
            let tree = check_tree(&document);
            if tree != 0 { return tree; }
            let head = node(&document, 0);
            match head {
                Option::Some(root) => {
                    let class = kind(&root);
                    match class {
                        Kind::Object => {
                            if root.start != 2 || root.end != input.len() - 1 || root.key_start != -1 { return 82; }
                            return check_values(&document);
                        },
                        Kind::Array => { return 83; },
                        Kind::String => { return 83; },
                        Kind::Number => { return 83; },
                        Kind::True => { return 83; },
                        Kind::False => { return 83; },
                        Kind::Null => { return 83; },
                    }
                },
                Option::None => { return 84; },
            }
        },
    }
}

fn parse_syntax_failure() -> i32 {
    let broken = b"[1,]";
    let mut input = new();
    if !resize_ok(try_extend(&mut input, &broken)) { return 85; }
    let result = parse(input);
    match result {
        ParseResult::Parsed(document) => { return 86; },
        ParseResult::Error(failure) => {
            let ParseFailure { input: returned, reason: why } = failure;
            if !same(&returned, &broken) { return 87; }
            match why {
                Validation::Syntax(offset) => { if offset == 3 { return 0; } return 88; },
                Validation::Valid => { return 89; },
                Validation::InvalidUtf8(offset) => { return 89; },
                Validation::CapacityOverflow => { return 89; },
                Validation::AllocationFailed => { return 89; },
                Validation::MemoryLimit => { return 89; },
            }
        },
    }
}

fn delimit_numbers(bytes: &mut [u8]) -> () {
    bytes[0] = 91u8;
    bytes[512] = 93u8;
    for slot in 1..256 { bytes[slot * 2] = 44u8; }
    return;
}

fn parse_memory_failure() -> i32 {
    let mut input = new();
    if !resize_ok(try_resize(&mut input, 513, 48u8)) { return 90; }
    delimit_numbers(&mut input);
    let result = parse(input);
    match result {
        ParseResult::Parsed(document) => { return 91; },
        ParseResult::Error(failure) => {
            let ParseFailure { input: returned, reason: why } = failure;
            if len(&returned) != 513 || !valid(&returned) { return 92; }
            match why {
                Validation::MemoryLimit => { return 0; },
                Validation::Syntax(offset) => { return 93; },
                Validation::Valid => { return 93; },
                Validation::InvalidUtf8(offset) => { return 93; },
                Validation::CapacityOverflow => { return 93; },
                Validation::AllocationFailed => { return 93; },
            }
        },
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
    let limited = string_limit();
    if limited != 0 { return limited; }
    let decoded = parsed_values();
    if decoded != 0 { return decoded; }
    let bad_json = parse_syntax_failure();
    if bad_json != 0 { return bad_json; }
    return parse_memory_failure();
}
