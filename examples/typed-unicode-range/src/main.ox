use alloc::bytes::len;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use std::io::write_some;
use std::io::WriteSomeStatus;
use crate::text::new_text;
use crate::text::append_char;
use crate::text::char_at;
use crate::text::byte_len;
use crate::text::into_bytes;
use crate::text::AppendStatus;
use crate::text::CharacterRead;

enum Number { Value(i32), Invalid }

fn argument_number(index: i32) -> Number {
    let mut bytes = b"........";
    let status = arg_bytes(index, &mut bytes);
    let mut length = 0;
    match status {
        ArgStatus::Copied(count) => { length = count; },
        ArgStatus::TooSmall(required) => { return Number::Invalid; },
        ArgStatus::Missing => { return Number::Invalid; },
        ArgStatus::TooLong => { return Number::Invalid; },
    }
    if length == 0 || length > 7 { return Number::Invalid; }
    let mut position = 0;
    let mut value = 0;
    while position < length {
        let next = bytes[position];
        let digit = next.to_i32() - 48;
        if digit < 0 || digit > 9 { return Number::Invalid; }
        if value > (1114112 - digit) / 10 { return Number::Invalid; }
        value = value * 10 + digit;
        position = position + 1;
    }
    return Number::Value(value);
}

fn print_range(first: i32, end: i32) -> i32 {
    if first > end { return 64; }
    let mut text = new_text();
    let mut code = first;
    while code < end {
        if code < 55296 || code > 57343 {
            let value = code.to_char_checked();
            let appended = append_char(&mut text, value);
            match appended {
                AppendStatus::Ok => {},
                AppendStatus::InvalidInput(offset) => { return 70; },
                AppendStatus::CapacityOverflow => { return 70; },
                AppendStatus::AllocationFailed => { return 70; },
                AppendStatus::MemoryLimit => { return 70; },
            }
        }
        code = code + 1;
    }
    let mut offset = 0;
    code = first;
    while code < end {
        if code < 55296 || code > 57343 {
            let read = char_at(&text, offset);
            match read {
                CharacterRead::Found(character) => {
                    let actual = character.value;
                    let scalar = actual.to_u32();
                    if scalar.to_i32_checked() != code { return 70; }
                    offset = character.next_byte;
                },
                CharacterRead::End => { return 70; },
                CharacterRead::InvalidOffset => { return 70; },
            }
        }
        code = code + 1;
    }
    if offset != byte_len(&text) { return 70; }
    let output = into_bytes(text);
    let length = len(&output);
    offset = 0;
    while offset < length {
        let written = write_some(&output[offset..length]);
        match written {
            WriteSomeStatus::Written(count) => {
                if count == 0 { return 74; }
                offset = offset + count;
            },
            WriteSomeStatus::Error(error) => {
                if error != 4 { return 74; }
            },
        }
    }
    return 0;
}

fn main() -> i32 {
    if arg_count() != 3 { return 64; }
    let start = argument_number(1);
    let finish = argument_number(2);
    match start {
        Number::Invalid => { return 64; },
        Number::Value(first) => {
            match finish {
                Number::Invalid => { return 64; },
                Number::Value(end) => { return print_range(first, end); },
            }
        },
    }
}
