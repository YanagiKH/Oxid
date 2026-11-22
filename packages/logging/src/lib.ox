use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_reserve;
use alloc::bytes::try_extend;
use crate::json::StringResult;
use crate::json::Validation;
use crate::json::quote;
use crate::text::Text;
use crate::text::append_to_bytes;
use crate::text::byte_len;
use std::time::unix_millis;

pub enum Level { Debug, Info, Warn, Error }

pub enum Line {
    Encoded(ByteBuf),
    InvalidMessage(i32),
    ClockUnavailable,
    CapacityOverflow,
    AllocationFailed,
    MemoryLimit,
}

struct Digits {
    bytes: [u8; 20],
    start: i32,
}

struct Label {
    bytes: [u8; 5],
    length: i32,
}

fn label(level: Level) -> Label {
    match level {
        Level::Debug => { return Label { bytes: b"debug", length: 5 }; },
        Level::Info => { return Label { bytes: b"info ", length: 4 }; },
        Level::Warn => { return Label { bytes: b"warn ", length: 4 }; },
        Level::Error => { return Label { bytes: b"error", length: 5 }; },
    }
}

fn decimal(value: i64) -> Digits {
    let mut bytes = b"00000000000000000000";
    let mut start = 20;
    let mut remaining = value;
    if value > 0i64 { remaining = 0i64 - value; }
    while true {
        let remainder = remaining % 10i64;
        let digit = 48i64 - remainder;
        start = start - 1;
        bytes[start] = digit.to_u8_checked();
        remaining = remaining / 10i64;
        if remaining == 0i64 { break; }
    }
    if value < 0i64 {
        start = start - 1;
        bytes[start] = 45u8;
    }
    return Digits { bytes: bytes, start: start };
}

fn appended(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn message_error(failure: Validation) -> Line {
    match failure {
        Validation::InvalidUtf8(offset) => { return Line::InvalidMessage(offset); },
        Validation::Syntax(offset) => { return Line::InvalidMessage(offset); },
        Validation::Valid => { return Line::InvalidMessage(0); },
        Validation::CapacityOverflow => { return Line::CapacityOverflow; },
        Validation::AllocationFailed => { return Line::AllocationFailed; },
        Validation::MemoryLimit => { return Line::MemoryLimit; },
    }
}

fn assemble(level: Level, timestamp: i64, quoted: &Text) -> Line {
    let time = decimal(timestamp);
    let name = label(level);
    let first = b"{\"timestamp_ms\":";
    let second = b",\"level\":\"";
    let third = b"\",\"message\":";
    let end = b"}\n";
    let fixed = first.len() + 20 - time.start + second.len() + name.length + third.len() + end.len();
    let size = byte_len(&*quoted);
    if size > 2147483647 - fixed { return Line::CapacityOverflow; }

    let mut output = new();
    let capacity = try_reserve(&mut output, fixed + size);
    match capacity {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return Line::CapacityOverflow; },
        GrowStatus::AllocationFailed => { return Line::AllocationFailed; },
        GrowStatus::MemoryLimit => { return Line::MemoryLimit; },
    }
    if !appended(try_extend(&mut output, &first))
        || !appended(try_extend(&mut output, &time.bytes[time.start..20]))
        || !appended(try_extend(&mut output, &second))
        || !appended(try_extend(&mut output, &name.bytes[0..name.length]))
        || !appended(try_extend(&mut output, &third))
        || !appended(append_to_bytes(&*quoted, &mut output))
        || !appended(try_extend(&mut output, &end)) {
        return Line::AllocationFailed;
    }
    return Line::Encoded(output);
}

pub fn encode_at(level: Level, timestamp_ms: i64, message: &[u8]) -> Line {
    let field = quote(&*message);
    match field {
        StringResult::Value(quoted) => { return assemble(level, timestamp_ms, &quoted); },
        StringResult::Error(failure) => { return message_error(failure); },
    }
}

pub fn encode(level: Level, message: &[u8]) -> Line {
    let timestamp = unix_millis();
    if timestamp == -9223372036854775808i64 { return Line::ClockUnavailable; }
    return encode_at(level, timestamp, &*message);
}
