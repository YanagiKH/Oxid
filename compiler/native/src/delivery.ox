use alloc::bytes::ByteBuf;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use alloc::bytes::truncate;
use alloc::vec::len as vector_len;
use alloc::vec::try_push;
use crate::model::Compiler;
use crate::model::SourceModule;
use crate::model::code as byte_code;
use crate::model::grown;
use crate::model::new_compiler;
use crate::model::token;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::OpenResult;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::write_some;

pub enum Input { Loaded(Compiler), Failed(i32) }
enum FileInput { Loaded(ByteBuf), Failed(i32) }

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
    let input = load_file(&path, 65536);
    match input {
        FileInput::Loaded(bytes) => { return Input::Loaded(new_compiler(bytes, path)); },
        FileInput::Failed(status_code) => { return Input::Failed(status_code); },
    }
}

fn load_file(path: &[u8], remaining: i32) -> FileInput {
    let opened = open_read(&*path);
    match opened {
        OpenResult::Opened(file) => {
            let mut handle = file;
            let mut source = new();
            let mut chunk = new();
            if !grown(try_resize(&mut chunk, 4096, 0u8)) { return FileInput::Failed(68); }
            while true {
                let got = read_some(&mut handle, &mut chunk);
                match got {
                    ReadSomeStatus::Data(data_size) => {
                        if data_size <= 0 || data_size > 4096 { return FileInput::Failed(74); }
                        if len(&source) > remaining - data_size { return FileInput::Failed(68); }
                        if !grown(try_extend(&mut source, &chunk[0..data_size])) { return FileInput::Failed(68); }
                    },
                    ReadSomeStatus::Eof => { return FileInput::Loaded(source); },
                    ReadSomeStatus::Error(read_code) => { if read_code != 4 { return FileInput::Failed(66); } },
                }
            }
        },
        OpenResult::Error(open_code) => { return FileInput::Failed(66); },
        OpenResult::MemoryLimit => { return FileInput::Failed(68); },
        OpenResult::AllocationFailed => { return FileInput::Failed(68); },
    }
    return FileInput::Failed(74);
}

pub fn load_module(c: &mut Compiler, name: i32) -> () {
    let extension = b".ox";
    let spelling = token(&*c, name);
    let mut directory_end = len(&*c.source_path);
    while directory_end > 0 && byte_code(&*c.source_path, directory_end - 1) != 47 {
        directory_end = directory_end - 1;
    }
    if directory_end + spelling.end - spelling.start + 3 > 4096 {
        c.error = 68;
        return;
    }
    let mut path = new();
    if !grown(try_extend(&mut path, &*c.source_path[0..directory_end]))
        || !grown(try_extend(&mut path, &*c.source[spelling.start..spelling.end]))
        || !grown(try_extend(&mut path, &extension)) {
        c.error = 68;
        return;
    }
    let start = len(&*c.source);
    let input = load_file(&path, 65536 - start);
    match input {
        FileInput::Failed(status_code) => { c.error = status_code; },
        FileInput::Loaded(bytes) => {
            if !grown(try_extend(&mut *c.source, &bytes)) { c.error = 68; return; }
            let first = vector_len(&*c.tokens);
            let end = len(&*c.source);
            crate::lexer::scan_range(&mut *c, start, end);
            if c.error != 0 { return; }
            if !grown(try_push(&mut *c.sources, SourceModule { name: name, first_token: first })) {
                c.error = 68;
            }
        },
    }
    return;
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
