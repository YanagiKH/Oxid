use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use vec::I32Vec;
use vec::Element;
use vec::FromBytes;
use vec::len;
use vec::get;
use vec::set;
use vec::from_le_bytes;
use vec::into_le_bytes;

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

fn insert_at(values: &mut I32Vec, index: i32) -> bool {
    let current = get(&*values, index);
    match current {
        Element::Missing => { return false; },
        Element::Value(key) => {
            let mut hole = index;
            while hole > 0 {
                let previous = get(&*values, hole - 1);
                match previous {
                    Element::Missing => { return false; },
                    Element::Value(before) => {
                        if before <= key { break; }
                        if !set(&mut *values, hole, before) { return false; }
                        hole = hole - 1;
                    },
                }
            }
            return set(&mut *values, hole, key);
        },
    }
}

fn insertion_sort(values: &mut I32Vec) -> bool {
    let count = len(&*values);
    let mut index = 1;
    while index < count {
        if !insert_at(&mut *values, index) { return false; }
        index = index + 1;
    }
    return true;
}

fn emit(values: I32Vec) -> i32 {
    let output = into_le_bytes(values);
    let written = write_all(&output);
    match written {
        WrittenAll::Ok => { return 0; },
        WrittenAll::Error(code) => { return 74; },
        WrittenAll::WriteZero => { return 74; },
    }
}

fn sort_records(input: ByteBuf) -> i32 {
    let converted = from_le_bytes(input);
    match converted {
        FromBytes::Ok(parsed) => {
            let mut values = parsed;
            if !insertion_sort(&mut values) { return 70; }
            return emit(values);
        },
        FromBytes::Invalid(rejected) => { return 65; },
    }
}

fn main() -> i32 {
    let loaded = read_all();
    match loaded {
        Loaded::Ok(input) => { return sort_records(input); },
        Loaded::CapacityOverflow => { return 70; },
        Loaded::AllocationFailed => { return 71; },
        Loaded::MemoryLimit => { return 71; },
        Loaded::IoError(code) => { return 74; },
    }
}
