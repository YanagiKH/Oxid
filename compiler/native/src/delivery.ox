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
use crate::model::word;
use crate::model::source_byte_limit;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::OpenResult;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::write_some;
use std::io::write_stderr;

pub enum Input { Loaded(Compiler), Failed(i32) }
enum FileInput { Loaded(ByteBuf), Failed(i32) }

fn target_code(flag: &[u8]) -> i32 {
    let size = flag.len();
    let linux_intel = b"--target=x86_64-unknown-linux-gnu";
    let linux_arm = b"--target=aarch64-unknown-linux-gnu";
    let macos_intel = b"--target=x86_64-apple-macosx11.0.0";
    let macos_arm = b"--target=arm64-apple-macosx11.0.0";
    if word(&*flag, 0, size, &linux_intel) { return 0; }
    if word(&*flag, 0, size, &linux_arm) { return 1; }
    if word(&*flag, 0, size, &macos_intel) { return 2; }
    if word(&*flag, 0, size, &macos_arm) { return 3; }
    return -1;
}

pub fn load() -> Input {
    let argc = arg_count();
    if argc < 2 || argc > 4 { return Input::Failed(64); }
    let mut path = new();
    if !grown(try_resize(&mut path, 4096, 0u8)) { return Input::Failed(68); }
    let mut platform = 0;
    if argc >= 3 {
        let target = arg_bytes(2, &mut path);
        match target {
            ArgStatus::Copied(size) => {
                platform = target_code(&path[0..size]);
                if platform < 0 { return Input::Failed(64); }
            },
            ArgStatus::TooSmall(required) => { return Input::Failed(64); },
            ArgStatus::Missing => { return Input::Failed(64); },
            ArgStatus::TooLong => { return Input::Failed(64); },
        }
    }
    let process_entry = argc == 4;
    if process_entry {
        let process = b"--entry-mode=process";
        let mode = arg_bytes(3, &mut path);
        match mode {
            ArgStatus::Copied(size) => {
                if !word(&path, 0, size, &process) { return Input::Failed(64); }
            },
            ArgStatus::TooSmall(required) => { return Input::Failed(64); },
            ArgStatus::Missing => { return Input::Failed(64); },
            ArgStatus::TooLong => { return Input::Failed(64); },
        }
    }
    let status = arg_bytes(1, &mut path);
    match status {
        ArgStatus::Copied(copied_size) => { truncate(&mut path, copied_size); },
        ArgStatus::TooSmall(required_size) => { return Input::Failed(64); },
        ArgStatus::Missing => { return Input::Failed(64); },
        ArgStatus::TooLong => { return Input::Failed(64); },
    }
    let input = load_file(&path, source_byte_limit());
    match input {
        FileInput::Loaded(bytes) => { return Input::Loaded(new_compiler(bytes, path, platform, process_entry)); },
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

fn append_parent_path(c: &Compiler, parent: i32, path: &mut ByteBuf) -> bool {
    if parent == 0 { return true; }
    let row = c.sources[parent];
    if !append_parent_path(&*c, row.parent, &mut *path) { return false; }
    let slash = b"/";
    let spelling = token(&*c, row.name);
    if len(&*path) + spelling.end - spelling.start + 1 > 4093 { return false; }
    return grown(try_extend(&mut *path, &*c.source[spelling.start..spelling.end]))
        && grown(try_extend(&mut *path, &slash));
}

fn append_module_path(c: &Compiler, parent: i32, name: i32, path: &mut ByteBuf) -> bool {
    let extension = b".ox";
    let spelling = token(&*c, name);
    let size = spelling.end - spelling.start;
    let mut directory_end = len(&*c.source_path);
    while directory_end > 0 && byte_code(&*c.source_path, directory_end - 1) != 47 {
        directory_end = directory_end - 1;
    }
    return size <= 255 && directory_end + size + 3 <= 4096
        && grown(try_extend(&mut *path, &*c.source_path[0..directory_end]))
        && append_parent_path(&*c, parent, &mut *path) && len(&*path) + size + 3 <= 4096
        && grown(try_extend(&mut *path, &*c.source[spelling.start..spelling.end]))
        && grown(try_extend(&mut *path, &extension));
}

pub fn load_module(c: &mut Compiler, parent: i32, name: i32, public: bool) -> () {
    let mut path = new();
    if !append_module_path(&*c, parent, name, &mut path) {
        c.error = 68;
        return;
    }
    if word(&*c.source_path, 0, len(&*c.source_path), &path) {
        c.error = 65;
        let spelling = token(&*c, name);
        c.error_start = spelling.start;
        return;
    }
    let start = len(&*c.source);
    let input = load_file(&path, source_byte_limit() - start);
    match input {
        FileInput::Failed(status_code) => { c.error = status_code; },
        FileInput::Loaded(bytes) => {
            if !grown(try_extend(&mut *c.source, &bytes)) { c.error = 68; return; }
            let first = vector_len(&*c.tokens);
            let end = len(&*c.source);
            let owner = c.sources[parent];
            let row = SourceModule { name: name, first_token: first, first_byte: start, parent: parent,
                depth: owner.depth + 1, public: public };
            let module = vector_len(&*c.sources);
            if !grown(try_push(&mut *c.sources, row)) {
                c.error = 68; return;
            }
            c.error_module = module;
            crate::lexer::scan_range(&mut *c, start, end);
            if c.error == 0 { c.error_module = parent; }
        },
    }
    return;
}

fn write_part(bytes: &[u8], diagnostic: bool) -> WriteSomeStatus {
    if diagnostic { return write_stderr(&*bytes); }
    return write_some(&*bytes);
}

fn write_to(bytes: &[u8], diagnostic: bool) -> i32 {
    let mut offset = 0;
    while offset < bytes.len() {
        let written = write_part(&*bytes[offset..bytes.len()], diagnostic);
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

pub fn write_all(bytes: &[u8]) -> i32 { return write_to(&*bytes, false); }

fn write_path(path: &[u8]) -> () {
    let replacement = b"?";
    let mut from = 0;
    for index in 0..path.len() {
        let byte = path[index];
        if byte < 32u8 || byte == 127u8 {
            if write_to(&*path[from..index], true) != 0 || write_to(&replacement, true) != 0 { return; }
            from = index + 1;
        }
    }
    write_to(&*path[from..path.len()], true);
    return;
}

fn write_number(number: i32) -> () {
    let mut digits = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let mut remaining = number;
    let mut start = 10;
    while remaining > 0 {
        start = start - 1;
        let ascii = remaining % 10 + 48;
        digits[start] = ascii.to_u8_checked();
        remaining = remaining / 10;
    }
    write_to(&digits[start..10], true);
    return;
}

pub fn report_status(status: i32) -> i32 {
    if status == 64 {
        let text = b"usage: oxid-scalar-compiler <source.ox> [--target=<triple>] [--entry-mode=process]\n";
        write_to(&text, true);
    }
    if status == 66 { let text = b"error: could not read Oxid source\n"; write_to(&text, true); }
    if status == 68 { let text = b"error: compiler resource limit exceeded\n"; write_to(&text, true); }
    if status == 74 { let text = b"error: compiler I/O failed\n"; write_to(&text, true); }
    return status;
}

pub fn report(c: &Compiler) -> i32 {
    if c.error != 65 { return report_status(c.error); }
    let mut first = 0;
    let mut path = new();
    let mut use_module = false;
    if c.error_module > 0 {
        let source = c.sources[c.error_module];
        first = source.first_byte;
        use_module = append_module_path(&*c, source.parent, source.name, &mut path);
    }
    if use_module { write_path(&path); }
    else { write_path(&*c.source_path); }
    let separator = b":";
    let diagnostic = b": error: invalid or unsupported Oxid source\n";
    let mut line = 1;
    let mut column = 1;
    while first < c.error_start {
        if byte_code(&*c.source, first) == 10 { line = line + 1; column = 1; }
        else { column = column + 1; }
        first = first + 1;
    }
    write_to(&separator, true);
    write_number(line);
    write_to(&separator, true);
    write_number(column);
    write_to(&diagnostic, true);
    return c.error;
}
