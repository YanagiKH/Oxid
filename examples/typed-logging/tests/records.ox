use alloc::bytes::GrowStatus;
use alloc::bytes::len;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use crate::logging::Level;
use crate::logging::Line;
use crate::logging::FieldsResult;
use crate::logging::encode_at;
use crate::logging::encode_at_with_fields;
use crate::logging::parse_fields;

fn equals(bytes: &[u8], expected: &[u8]) -> bool {
    if bytes.len() != expected.len() { return false; }
    for position in 0..bytes.len() {
        if bytes[position] != expected[position] { return false; }
    }
    return true;
}

fn is_record(line: Line, expected: &[u8]) -> bool {
    match line {
        Line::Encoded(bytes) => { return equals(&bytes, &*expected); },
        Line::InvalidMessage(offset) => { return false; },
        Line::ClockUnavailable => { return false; },
        Line::CapacityOverflow => { return false; },
        Line::AllocationFailed => { return false; },
        Line::MemoryLimit => { return false; },
    }
}

fn reports_memory_limit() -> bool {
    let mut bytes = new();
    let initialized = try_resize(&mut bytes, 2048, 120u8);
    match initialized {
        GrowStatus::Ok => {},
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
    let result = encode_at(Level::Info, 10i64, &bytes);
    match result {
        Line::MemoryLimit => { return len(&bytes) == 2048; },
        Line::Encoded(output) => { return false; },
        Line::InvalidMessage(offset) => { return false; },
        Line::ClockUnavailable => { return false; },
        Line::CapacityOverflow => { return false; },
        Line::AllocationFailed => { return false; },
    }
}

fn signed_minimum() -> bool {
    let message = b"one\n\"two\"\\\xE9\x9B\xAA\x00";
    let expected = b"{\"timestamp_ms\":-9223372036854775808,\"level\":\"warn\",\"message\":\"one\\u000a\\\"two\\\"\\\\\xE9\x9B\xAA\\u0000\"}\n";
    let warning = encode_at(Level::Warn, -9223372036854775808i64, &message);
    return is_record(warning, &expected);
}

fn signed_maximum() -> bool {
    let empty = b"";
    let maximum = b"{\"timestamp_ms\":9223372036854775807,\"level\":\"error\",\"message\":\"\"}\n";
    let failure = encode_at(Level::Error, 9223372036854775807i64, &empty);
    return is_record(failure, &maximum);
}

fn zero_timestamp() -> bool {
    let empty = b"";
    let zero = b"{\"timestamp_ms\":0,\"level\":\"debug\",\"message\":\"\"}\n";
    let debug_line = encode_at(Level::Debug, 0i64, &empty);
    return is_record(debug_line, &zero);
}

fn invalid_fields(input: &[u8], expected: i32) -> bool {
    let parsed = parse_fields(&*input);
    match parsed {
        FieldsResult::Invalid(offset) => { return offset == expected; },
        FieldsResult::Ready(fields) => { return false; },
        FieldsResult::CapacityOverflow => { return false; },
        FieldsResult::AllocationFailed => { return false; },
        FieldsResult::MemoryLimit => { return false; },
    }
}

fn structured_fields() -> bool {
    let list = b"[1,2]";
    let multiline = b"{}\n{\"extra\":true}";
    let broken = b"{\"a\":\"\xFF\"}";
    if !invalid_fields(&list, 0) || !invalid_fields(&multiline, 2) || !invalid_fields(&broken, 6) {
        return false;
    }
    let data = b"{\"attempt\":2,\"details\":{\"name\":\"\xE9\x9B\xAA\\n\",\"ready\":true}}";
    let selected = parse_fields(&data);
    match selected {
        FieldsResult::Ready(fields) => {
            let message = b"job complete";
            let expected = b"{\"timestamp_ms\":8,\"level\":\"info\",\"message\":\"job complete\",\"fields\":{\"attempt\":2,\"details\":{\"name\":\"\xE9\x9B\xAA\\n\",\"ready\":true}}}\n";
            let first = encode_at_with_fields(Level::Info, 8i64, &message, &fields);
            if !is_record(first, &expected) { return false; }
            let second = encode_at_with_fields(Level::Info, 8i64, &message, &fields);
            return is_record(second, &expected);
        },
        FieldsResult::Invalid(offset) => { return false; },
        FieldsResult::CapacityOverflow => { return false; },
        FieldsResult::AllocationFailed => { return false; },
        FieldsResult::MemoryLimit => { return false; },
    }
}

fn main() -> i32 {
    if !reports_memory_limit() { return 10; }
    if !signed_minimum() { return 1; }
    if !signed_maximum() { return 2; }
    if !zero_timestamp() { return 3; }
    if !structured_fields() { return 11; }
    let bad = b"x\xFF";
    let invalid = encode_at(Level::Info, 0i64, &bad);
    match invalid {
        Line::InvalidMessage(offset) => {
            if offset == 1 { return 0; }
            return 4;
        },
        Line::Encoded(bytes) => { return 5; },
        Line::ClockUnavailable => { return 6; },
        Line::CapacityOverflow => { return 7; },
        Line::AllocationFailed => { return 8; },
        Line::MemoryLimit => { return 9; },
    }
}
