use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::truncate;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use std::env::ArgStatus;
use std::env::arg_bytes;
use std::env::arg_count;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use std::io::write_stderr;
use crate::text::Text;
use crate::text::FromBytes;
use crate::text::PrefixValidation;
use crate::text::from_bytes;
use crate::text::into_bytes;
use crate::text::validate_prefix;

enum Loaded {
    Ok(ByteBuf), CapacityOverflow, AllocationFailed, MemoryLimit, IoError(i32)
}
enum WrittenAll { Ok, Error(i32), WriteZero }
enum Mode { Whole, Stream, Help, Invalid }

fn read_all() -> Loaded {
    let mut bytes = new();
    let mut chunk = new();
    let zero = 0;
    let initialized = try_resize(&mut chunk, 1024, zero.to_u8_checked());
    match initialized {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return Loaded::CapacityOverflow; },
        GrowStatus::AllocationFailed => { return Loaded::AllocationFailed; },
        GrowStatus::MemoryLimit => { return Loaded::MemoryLimit; },
    }
    while true {
        let read = read_some(&mut chunk);
        match read {
            ReadSomeStatus::Data(count) => {
                let appended = try_extend(&mut bytes, &chunk[0..count]);
                match appended {
                    GrowStatus::Ok => {},
                    GrowStatus::CapacityOverflow => { return Loaded::CapacityOverflow; },
                    GrowStatus::AllocationFailed => { return Loaded::AllocationFailed; },
                    GrowStatus::MemoryLimit => { return Loaded::MemoryLimit; },
                }
            },
            ReadSomeStatus::Eof => { return Loaded::Ok(bytes); },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return Loaded::IoError(code); }
            },
        }
    }
    return Loaded::IoError(5);
}

fn deliver(bytes: &[u8], errors: bool) -> WriteSomeStatus {
    if errors { return write_stderr(&*bytes); }
    return write_some(&*bytes);
}

fn write_all(bytes: &[u8], errors: bool) -> WrittenAll {
    let mut offset = 0;
    let length = bytes.len();
    while offset < length {
        let written = deliver(&*bytes[offset..length], errors);
        match written {
            WriteSomeStatus::Written(count) => {
                if count == 0 { return WrittenAll::WriteZero; }
                offset = offset + count;
            },
            WriteSomeStatus::Error(code) => {
                if code != 4 { return WrittenAll::Error(code); }
            },
        }
    }
    return WrittenAll::Ok;
}

fn normalize(document: Text) -> ByteBuf {
    let mut input = into_bytes(document);
    let length = normalize_bytes(&mut input);
    truncate(&mut input, length);
    return input;
}

fn normalize_bytes(input: &mut [u8]) -> i32 {
    let mut written = 0;
    let length = input.len();
    while written < length {
        if input[written] == 13u8 { break; }
        written = written + 1;
    }
    let mut offset = written;
    while offset < length {
        let value = input[offset];
        let number = value.to_i32();
        let mut output_byte = value;
        if number == 13 {
            let newline = 10;
            output_byte = newline.to_u8_checked();
        }
        input[written] = output_byte;
        written = written + 1;
        offset = offset + 1;
        if number == 13 && offset < length {
            let following = input[offset];
            if following.to_i32() == 10 { offset = offset + 1; }
        }
    }
    return written;
}

fn emit(document: Text) -> i32 {
    let output = normalize(document);
    let written = write_all(&output, false);
    match written {
        WrittenAll::Ok => { return 0; },
        WrittenAll::Error(code) => { return 74; },
        WrittenAll::WriteZero => { return 74; },
    }
}

fn convert(input: ByteBuf) -> i32 {
    let converted = from_bytes(input);
    match converted {
        FromBytes::Ok(document) => { return emit(document); },
        FromBytes::Invalid(rejected) => { return 65; },
    }
}

fn is_flag(input: &[u8], flag: &[u8]) -> bool {
    if input.len() != flag.len() { return false; }
    for index in 0..flag.len() {
        if input[index] != flag[index] { return false; }
    }
    return true;
}

fn mode() -> Mode {
    let argc = arg_count();
    if argc == 1 { return Mode::Whole; }
    if argc != 2 { return Mode::Invalid; }
    let mut received = b"        ";
    let copied = arg_bytes(1, &mut received);
    match copied {
        ArgStatus::Copied(count) => {
            let stream_flag = b"--stream";
            let help_flag = b"--help";
            if is_flag(&received[0..count], &stream_flag) { return Mode::Stream; }
            if is_flag(&received[0..count], &help_flag) { return Mode::Help; }
        },
        ArgStatus::Missing => {},
        ArgStatus::TooSmall(required) => {},
        ArgStatus::TooLong => {},
    }
    return Mode::Invalid;
}

fn usage(errors: bool) -> i32 {
    let words = b"Usage: utf8-normalize [--stream|--help] < input > output\n";
    let written = write_all(&words, errors);
    match written {
        WrittenAll::Ok => {
            if errors { return 64; }
            return 0;
        },
        WrittenAll::Error(code) => { return 74; },
        WrittenAll::WriteZero => { return 74; },
    }
}

fn write_chunk(input: &mut [u8], previous_cr: bool) -> i32 {
    let limit = input.len();
    let mut start = 0;
    if previous_cr && input[0] == 10u8 { start = 1; }
    let length = normalize_bytes(&mut *input[start..limit]);
    let end = start + length;
    let written = write_all(&*input[start..end], false);
    match written {
        WrittenAll::Ok => { return 0; },
        WrittenAll::Error(code) => { return 74; },
        WrittenAll::WriteZero => { return 74; },
    }
}

fn stream_chunks(buffer: &mut [u8]) -> i32 {
    let size = buffer.len();
    let mut carry = 0;
    let mut previous_cr = false;
    while true {
        let read = read_some(&mut *buffer[carry..size]);
        match read {
            ReadSomeStatus::Data(count) => {
                let extent = carry + count;
                let checked = validate_prefix(&*buffer[0..extent]);
                let mut prefix = extent;
                match checked {
                    PrefixValidation::Complete => {},
                    PrefixValidation::Incomplete(offset) => { prefix = offset; },
                    PrefixValidation::Invalid(offset) => { return 65; },
                }
                if prefix > 0 {
                    let last = buffer[prefix - 1];
                    let status = write_chunk(&mut *buffer[0..prefix], previous_cr);
                    if status != 0 { return status; }
                    previous_cr = last == 13u8;
                }
                carry = extent - prefix;
                for index in 0..carry {
                    let byte = buffer[prefix + index];
                    buffer[index] = byte;
                }
            },
            ReadSomeStatus::Eof => {
                if carry != 0 { return 65; }
                return 0;
            },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return 74; }
            },
        }
    }
    return 74;
}

fn stream() -> i32 {
    let mut buffer = new();
    let allocated = try_resize(&mut buffer, 4096, 0u8);
    match allocated {
        GrowStatus::Ok => { return stream_chunks(&mut buffer); },
        GrowStatus::CapacityOverflow => { return 70; },
        GrowStatus::AllocationFailed => { return 71; },
        GrowStatus::MemoryLimit => { return 71; },
    }
}

fn main() -> i32 {
    let selected = mode();
    match selected {
        Mode::Whole => {},
        Mode::Stream => { return stream(); },
        Mode::Help => { return usage(false); },
        Mode::Invalid => { return usage(true); },
    }
    let loaded = read_all();
    match loaded {
        Loaded::Ok(input) => { return convert(input); },
        Loaded::CapacityOverflow => { return 70; },
        Loaded::AllocationFailed => { return 71; },
        Loaded::MemoryLimit => { return 71; },
        Loaded::IoError(code) => { return 74; },
    }
}
