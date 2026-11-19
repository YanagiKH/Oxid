use alloc::bytes::ByteBuf;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use alloc::bytes::truncate;
use crate::model::grown;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::OpenResult;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::write_some;

pub enum Input { Loaded(ByteBuf), Failed(i32) }
pub fn load() -> Input {
    if arg_count() != 2 { return Input::Failed(64); }
    let mut path = new();
    if !grown(try_resize(&mut path, 4096, 0u8)) { return Input::Failed(68); }
    let status = arg_bytes(1, &mut path);
    match status {
        ArgStatus::Copied(copied_size) => { truncate(&mut path, copied_size); },
        ArgStatus::TooSmall(required_size) => { return Input::Failed(64); },
        ArgStatus::Missing => { return Input::Failed(64); },
        ArgStatus::TooLong => { return Input::Failed(64); },
    }
    let opened = open_read(&path);
    match opened {
        OpenResult::Opened(file) => {
            let mut handle = file;
            let mut source = new();
            let mut chunk = new();
            if !grown(try_resize(&mut chunk, 4096, 0u8)) { return Input::Failed(68); }
            while true {
                let got = read_some(&mut handle, &mut chunk);
                match got {
                    ReadSomeStatus::Data(data_size) => {
                        if data_size <= 0 || data_size > 4096 { return Input::Failed(74); }
                        if len(&source) > 65536 - data_size { return Input::Failed(68); }
                        if !grown(try_extend(&mut source, &chunk[0..data_size])) { return Input::Failed(68); }
                    },
                    ReadSomeStatus::Eof => { return Input::Loaded(source); },
                    ReadSomeStatus::Error(read_code) => { if read_code != 4 { return Input::Failed(66); } },
                }
            }
        },
        OpenResult::Error(open_code) => { return Input::Failed(66); },
        OpenResult::MemoryLimit => { return Input::Failed(68); },
        OpenResult::AllocationFailed => { return Input::Failed(68); },
    }
    return Input::Failed(74);
}
pub fn write_all(bytes: &[u8]) -> i32 {
    let mut offset = 0;
    while offset < bytes.len() {
        let written = write_some(&*bytes[offset..bytes.len()]);
        match written {
            WriteSomeStatus::Written(size) => {
                if size <= 0 || size > bytes.len() - offset { return 74; }
                offset = offset + size;
            },
            WriteSomeStatus::Error(code) => { if code != 4 { return 74; } },
        }
    }
    return 0;
}
