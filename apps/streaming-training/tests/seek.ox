use crate::reader::open_at;
use crate::reader::next;
use crate::reader::remaining;
use crate::reader::position;
use crate::reader::Result;
use crate::reader::Error;
use crate::tensor::TensorResult;
use crate::tensor::get;
use crate::tensor::row_count;
use std::env::arg_bytes;
use std::env::ArgStatus;

fn argument(index: i32) -> i32 {
    let mut bytes = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let copied = arg_bytes(index, &mut bytes);
    match copied {
        ArgStatus::Copied(length) => {
            if length == 0 { return -1; }
            let mut value = 0;
            for offset in 0..length { let byte = bytes[offset]; if byte < 48u8 || byte > 57u8 { return -1; } let digit = byte.to_i32_checked() - 48; if value > (2147483647 - digit) / 10 { return -1; } value = value * 10 + digit; }
            return value;
        },
        ArgStatus::Missing => { return -1; }, ArgStatus::TooSmall(required) => { return -1; }, ArgStatus::TooLong => { return -1; },
    }
}
fn code(error: Error) -> i32 {
    match error {
        Error::InvalidHeader => { return 1; }, Error::Truncated => { return 2; }, Error::TrailingData => { return 3; }, Error::InvalidBatchSize => { return 4; }, Error::Poisoned => { return 5; }, Error::End => { return 6; }, Error::InvalidStartRow => { return 7; }, Error::Io(number) => { return 1000 + number; }, Error::Tensor(cause) => { return 9; },
    }
}
fn main() -> i32 {
    let start = argument(1); let rows = argument(2); let expected = argument(3); let size = argument(4);
    if start < 0 || rows < 0 || expected < 0 || size < 0 { return 90; }
    let path = b"input.oxds"; let opened = open_at(&path, size, start);
    match opened {
        Result::Err(problem) => { if expected != 0 && code(problem) == expected { return 0; } return 91; },
        Result::Ok(value) => {
            let mut reader = value; let mut seen = start;
            if position(&reader) != start || remaining(&reader) != rows - start { return 92; }
            while remaining(&reader) > 0 {
                let before = position(&reader); let loaded = next(&mut reader);
                match loaded {
                    Result::Err(problem) => {
                        if expected == 0 || code(problem) != expected || position(&reader) != before { return 93; }
                        let again = next(&mut reader);
                        match again { Result::Ok(batch) => { return 94; }, Result::Err(error) => { if code(error) != 5 || position(&reader) != before { return 95; } return 0; }, }
                    },
                    Result::Ok(batch) => {
                        for row in 0..row_count(&batch.inputs) {
                            let expected_first = seen.to_f64(); let expected_last = -2.0 * expected_first;
                            let first = get(&batch.inputs, row, 0); let last = get(&batch.targets, row, 0);
                            match first { TensorResult::Ok(number) => { if number.to_bits() != expected_first.to_bits() { return 96; } }, TensorResult::Err(error) => { return 97; }, }
                            match last { TensorResult::Ok(number) => { if number.to_bits() != expected_last.to_bits() { return 98; } }, TensorResult::Err(error) => { return 99; }, }
                            seen = seen + 1;
                        }
                        if position(&reader) != seen { return 100; }
                    },
                }
            }
            if expected != 0 || seen != rows { return 101; }
            let end = next(&mut reader); match end { Result::Ok(batch) => { return 102; }, Result::Err(error) => { if code(error) != 6 { return 103; } return 0; }, }
        },
    }
}
