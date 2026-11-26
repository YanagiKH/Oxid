use std::fs::File;
use std::fs::FileStatus;
use std::fs::OpenResult;
use std::fs::close;
use std::fs::open_regular_read;
use std::fs::read_some;
use std::io::ReadSomeStatus;
use std::process::termination_requested;
use std::process::wait_for_termination;

enum Count {
    Complete(u64),
    Io(i32),
    Memory,
    Overflow,
    Cancelled,
}

pub fn valid_path(path: &[u8]) -> bool {
    if path.len() == 0 { return false; }
    for index in 0..path.len() {
        if path[index] == 0u8 { return false; }
    }
    return true;
}

fn count_bytes(file: &mut File, buffer: &mut [u8]) -> Count {
    let mut total = 0u64;
    while !termination_requested() {
        let read = read_some(&mut *file, &mut *buffer);
        match read {
            ReadSomeStatus::Eof => { return Count::Complete(total); },
            ReadSomeStatus::Data(count) => {
                if count <= 0 { return Count::Io(5); }
                let amount = count.to_u64_checked();
                if total > 18446744073709551615u64 - amount { return Count::Overflow; }
                total = total + amount;
            },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return Count::Io(code); }
            },
        }
    }
    return Count::Cancelled;
}

fn emit(result: Count, attempts: i32) -> i32 {
    if termination_requested() { return 130; }
    let mut written = false;
    match result {
        Count::Complete(bytes) => { written = crate::report::success(bytes, attempts); },
        Count::Io(code) => { written = crate::report::io_failure(code, attempts); },
        Count::Memory => {
            let reason = b"out-of-memory";
            written = crate::report::failure(&reason, attempts);
        },
        Count::Overflow => {
            let reason = b"file-too-large";
            written = crate::report::failure(&reason, attempts);
        },
        Count::Cancelled => { return 130; },
    }
    if termination_requested() { return 130; }
    if !written { return 74; }
    return 0;
}

fn reject(reason: &[u8]) -> i32 {
    let written = crate::report::failure(&*reason, 0);
    if termination_requested() { return 130; }
    if !written { return 74; }
    return 0;
}

pub fn submit(path: &[u8], too_long: bool, buffer: &mut [u8]) -> i32 {
    if termination_requested() { return 130; }
    if too_long {
        let reason = b"path-too-long";
        return reject(&reason);
    }
    if !valid_path(&*path) {
        let reason = b"invalid-path";
        return reject(&reason);
    }
    let mut attempts = 0;
    while !termination_requested() {
        attempts = attempts + 1;
        let opened = open_regular_read(&*path);
        match opened {
            OpenResult::Opened(handle) => {
                let mut file = handle;
                let counted = count_bytes(&mut file, &mut *buffer);
                let closed = close(&mut file);
                match closed {
                    FileStatus::Ok => { return emit(counted, attempts); },
                    FileStatus::Error(code) => { return emit(Count::Io(code), attempts); },
                }
            },
            OpenResult::Error(code) => {
                if code != 2 || attempts == 3 { return emit(Count::Io(code), attempts); }
                if wait_for_termination(250) { return 130; }
            },
            OpenResult::MemoryLimit => { return emit(Count::Memory, attempts); },
            OpenResult::AllocationFailed => { return emit(Count::Memory, attempts); },
        }
    }
    return 130;
}
