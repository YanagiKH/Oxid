mod jobs;
mod report;

use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use std::io::ReadSomeStatus;
use std::io::read_some;
use std::process::enable_termination_notifications;
use std::process::termination_requested;
use std::process::wait_for_stdin;
use std::process::wait_for_termination;

fn serve(input: &mut [u8], path: &mut [u8], file_bytes: &mut [u8]) -> i32 {
    let mut used = 0;
    let mut too_long = false;
    while !termination_requested() {
        if wait_for_stdin(250) {
            let read = read_some(&mut *input);
            match read {
                ReadSomeStatus::Data(count) => {
                    for index in 0..count {
                        if termination_requested() { return 130; }
                        let byte = input[index];
                        if byte == 10u8 {
                            let mut length = used;
                            if length > 0 && path[length - 1] == 13u8 { length = length - 1; }
                            let status = crate::jobs::submit(&*path[0..length], too_long || length > 4095, &mut *file_bytes);
                            if status != 0 { return status; }
                            used = 0;
                            too_long = false;
                        } else {
                            if used == path.len() { too_long = true; }
                            else {
                                path[used] = byte;
                                used = used + 1;
                            }
                        }
                    }
                },
                ReadSomeStatus::Eof => {
                    if used == 0 { return 0; }
                    return crate::jobs::submit(&*path[0..used], too_long || used > 4095, &mut *file_bytes);
                },
                ReadSomeStatus::Error(code) => {
                    if code != 4 {
                        let problem = b"file-worker: could not read standard input\n";
                        crate::report::write(&problem, true);
                        return 74;
                    }
                },
            }
        } else {
            if wait_for_termination(25) { return 130; }
        }
    }
    return 130;
}

fn initialized(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn help_requested() -> bool {
    if arg_count() != 2 { return false; }
    let mut value = b"      ";
    let expected = b"--help";
    let copied = arg_bytes(1, &mut value);
    match copied {
        ArgStatus::Copied(length) => {
            if length != expected.len() { return false; }
            for index in 0..length {
                if value[index] != expected[index] { return false; }
            }
            return true;
        },
        ArgStatus::Missing => { return false; },
        ArgStatus::TooSmall(required) => { return false; },
        ArgStatus::TooLong => { return false; },
    }
}

fn main() -> i32 {
    if arg_count() != 1 {
        let usage = b"Usage: file-worker [--help] < paths.txt\n";
        if help_requested() {
            if crate::report::write(&usage, false) { return 0; }
            return 74;
        }
        crate::report::write(&usage, true);
        return 64;
    }
    if !enable_termination_notifications() {
        let problem = b"file-worker: could not enable shutdown notifications\n";
        crate::report::write(&problem, true);
        return 74;
    }
    let mut input = new();
    let mut path = new();
    let mut file_bytes = new();
    if !initialized(try_resize(&mut input, 4096, 0u8))
    || !initialized(try_resize(&mut path, 4096, 0u8))
    || !initialized(try_resize(&mut file_bytes, 32768, 0u8)) {
        let problem = b"file-worker: could not allocate buffers\n";
        crate::report::write(&problem, true);
        return 74;
    }
    return serve(&mut input, &mut path, &mut file_bytes);
}
