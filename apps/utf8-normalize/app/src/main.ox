use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::truncate;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use crate::text::Text;
use crate::text::FromBytes;
use crate::text::from_bytes;
use crate::text::append_to_bytes;

enum Normalized { Ok(ByteBuf), Error(i32) }

enum Loaded {
    Ok(ByteBuf), CapacityOverflow, AllocationFailed, MemoryLimit, IoError(i32)
}
enum WrittenAll { Ok, Error(i32), WriteZero }

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

fn write_all(bytes: &[u8]) -> WrittenAll {
    let mut offset = 0;
    let length = bytes.len();
    while offset < length {
        let written = write_some(&*bytes[offset..length]);
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


fn growth_code(status: GrowStatus) -> i32 {
    match status {
        GrowStatus::Ok => { return 0; },
        GrowStatus::CapacityOverflow => { return 70; },
        GrowStatus::AllocationFailed => { return 71; },
        GrowStatus::MemoryLimit => { return 71; },
    }
}

fn normalize(document: &Text) -> Normalized {
    let mut input = new();
    let copied = append_to_bytes(&*document, &mut input);
    let copy_code = growth_code(copied);
    if copy_code != 0 { return Normalized::Error(copy_code); }
    let length = normalize_bytes(&mut input);
    truncate(&mut input, length);
    return Normalized::Ok(input);
}

fn normalize_bytes(input: &mut [u8]) -> i32 {
    let mut offset = 0;
    let mut written = 0;
    let length = input.len();
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
    let normalized = normalize(&document);
    match normalized {
        Normalized::Error(code) => { return code; },
        Normalized::Ok(output) => {
            let written = write_all(&output);
            match written {
                WrittenAll::Ok => { return 0; },
                WrittenAll::Error(code) => { return 74; },
                WrittenAll::WriteZero => { return 74; },
            }
        },
    }
}

fn convert(input: ByteBuf) -> i32 {
    let converted = from_bytes(input);
    match converted {
        FromBytes::Ok(document) => { return emit(document); },
        FromBytes::Invalid(rejected) => { return 65; },
    }
}

fn main() -> i32 {
    let loaded = read_all();
    match loaded {
        Loaded::Ok(input) => { return convert(input); },
        Loaded::CapacityOverflow => { return 70; },
        Loaded::AllocationFailed => { return 71; },
        Loaded::MemoryLimit => { return 71; },
        Loaded::IoError(code) => { return 74; },
    }
}
