use std::io::WriteSomeStatus;
use std::io::write_some;
use std::io::write_stderr;
use std::process::termination_requested;

fn deliver(text: &[u8], errors: bool) -> WriteSomeStatus {
    if errors { return write_stderr(&*text); }
    return write_some(&*text);
}

pub fn write(text: &[u8], errors: bool) -> bool {
    let mut offset = 0;
    let length = text.len();
    while offset < length {
        if termination_requested() { return false; }
        let written = deliver(&*text[offset..length], errors);
        match written {
            WriteSomeStatus::Written(count) => {
                if count <= 0 { return false; }
                offset = offset + count;
            },
            WriteSomeStatus::Error(code) => {
                if code != 4 || termination_requested() { return false; }
            },
        }
    }
    return true;
}

pub fn decimal(number: u64, digits: &mut [u8]) -> i32 {
    let mut value = number;
    let mut start = digits.len();
    while start > 0 {
        let remainder = value % 10u64;
        start = start - 1;
        digits[start] = remainder.to_u8_checked() + 48u8;
        value = value / 10u64;
        if value == 0u64 { return start; }
    }
    return -1;
}

fn write_number(number: u64) -> bool {
    let mut digits = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let start = decimal(number, &mut digits);
    return write(&digits[start..20], false);
}

fn suffix(attempts: i32) -> bool {
    let space = b" ";
    let newline = b"\n";
    return write(&space, false) && write_number(attempts.to_u64_checked())
    && write(&newline, false);
}

pub fn success(bytes: u64, attempts: i32) -> bool {
    let prefix = b"OK ";
    return write(&prefix, false) && write_number(bytes) && suffix(attempts);
}

pub fn failure(reason: &[u8], attempts: i32) -> bool {
    let prefix = b"ERR ";
    return write(&prefix, false) && write(&*reason, false) && suffix(attempts);
}

pub fn io_failure(code: i32, attempts: i32) -> bool {
    let prefix = b"ERR io:";
    return write(&prefix, false) && write_number(code.to_u64_checked()) && suffix(attempts);
}
