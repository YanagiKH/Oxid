use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;

pub struct Document { pub bytes: ByteBuf }
pub enum Loaded {
    Ok(Document), CapacityOverflow, AllocationFailed, MemoryLimit, IoError(i32)
}
pub enum WrittenAll { Ok, Error(i32), WriteZero }

pub fn read_all() -> Loaded {
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
            ReadSomeStatus::Eof => { return Loaded::Ok(Document { bytes: bytes }); },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return Loaded::IoError(code); }
            },
        }
    }
    return Loaded::IoError(5);
}

pub fn write_all(bytes: &[u8]) -> WrittenAll {
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
