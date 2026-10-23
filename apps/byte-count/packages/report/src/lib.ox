use crate::counts::Totals;
use crate::counts::byte_count;
use crate::counts::line_count;
use std::io::write_stdout;
use std::io::WriteStatus;

fn write_byte(value: i32) -> i32 {
    let bytes = [value];
    let status = write_stdout(&bytes);
    match status {
        WriteStatus::Complete => { return 0; },
        WriteStatus::InvalidInput => { return 3; },
        WriteStatus::IoError(count) => { return 3; },
    }
}

fn write_decimal(value: i32) -> i32 {
    let mut divisor = 1000000000;
    while divisor > 1 && value / divisor == 0 {
        divisor = divisor / 10;
    }
    while divisor > 0 {
        let digit = (value / divisor) % 10;
        if write_byte(48 + digit) != 0 {
            return 3;
        }
        divisor = divisor / 10;
    }
    return 0;
}

pub fn write_report(totals: &Totals) -> i32 {
    if write_decimal(line_count(&*totals)) != 0 { return 3; }
    if write_byte(32) != 0 { return 3; }
    if write_decimal(byte_count(&*totals)) != 0 { return 3; }
    return write_byte(10);
}
