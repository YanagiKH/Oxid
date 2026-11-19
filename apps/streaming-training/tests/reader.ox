use crate::tensor::Tensor;
use crate::tensor::TensorResult;
use crate::tensor::Error as TensorError;
use crate::tensor::get;
use crate::tensor::row_count;
use crate::tensor::column_count;
use crate::reader::Result;
use crate::reader::Error;
use crate::reader::open;
use crate::reader::open_at;
use crate::reader::next;
use crate::reader::remaining;
use crate::reader::position;
use crate::reader::feature_count;
use crate::reader::target_count;
use std::env::arg_count;
use std::io::read_some;
use std::io::ReadSomeStatus;
use std::io::write_some;
use std::io::WriteSomeStatus;
use std::env::arg_bytes;
use std::env::ArgStatus;

fn argument(index: i32) -> i32 {
    let mut buffer = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let copied = arg_bytes(index, &mut buffer);
    match copied { ArgStatus::Copied(length) => { let mut number = 0; for offset in 0..length { let byte = buffer[offset]; if byte < 48u8 || byte > 57u8 { return -1; } let digit = byte.to_i32_checked() - 48; if number > (2147483647 - digit) / 10 { return -1; } number = number * 10 + digit; } return number; }, ArgStatus::TooSmall(required) => { return -1; }, ArgStatus::Missing => { return -1; }, ArgStatus::TooLong => { return -1; }, }
}
fn failure_code(problem: Error) -> i32 {
    match problem {
        Error::InvalidHeader => { return 1; }, Error::Truncated => { return 2; }, Error::TrailingData => { return 3; }, Error::InvalidBatchSize => { return 4; }, Error::Poisoned => { return 5; }, Error::End => { return 6; }, Error::InvalidStartRow => { return 7; }, Error::Io(code) => { return 1000 + code; },
        Error::Tensor(cause) => { match cause {
                TensorError::MemoryLimit => { return 8; }, TensorError::NonFinite => { return 9; }, TensorError::SizeOverflow => { return 10; },
                TensorError::NegativeShape => { return 20; }, TensorError::ShapeMismatch => { return 21; }, TensorError::Bounds => { return 22; }, TensorError::AllocationFailed => { return 23; }, TensorError::InvalidStorage => { return 24; }, TensorError::EmptyReduction => { return 25; }, TensorError::InvalidNode => { return 26; }, TensorError::GraphCapacity => { return 27; }, TensorError::InvalidRate => { return 28; }, TensorError::InvalidOptimizer => { return 29; }, TensorError::StepOverflow => { return 30; }, TensorError::InvalidCheckpoint => { return 31; }, TensorError::IoError(code) => { return 2000 + code; }, TensorError::WriteZero => { return 32; }, } },
    }
}
fn wait_for_inspection() -> bool {
    let marker = b"READY\n"; let written = write_some(&marker); match written { WriteSomeStatus::Written(count) => { if count != 6 { return false; } }, WriteSomeStatus::Error(code) => { return false; }, }
    let mut buffer = [0u8]; let read = read_some(&mut buffer); match read { ReadSomeStatus::Data(count) => { return count == 1; }, ReadSomeStatus::Eof => { return false; }, ReadSomeStatus::Error(code) => { return false; }, }
}
fn verify_row(inputs: &Tensor, targets: &Tensor, row: i32, seen: i32) -> i32 {
    let reference = seen.to_f64();
    for column in 0..column_count(&*inputs) {
        let expected = reference + 0.5 * column.to_f64();
        let first = get(&*inputs, row, column); match first { TensorResult::Ok(number) => { if number.to_bits() != expected.to_bits() { return 96; } }, TensorResult::Err(problem) => { return 97; }, }
    }
    for column in 0..column_count(&*targets) {
        let expected = -2.0 * reference - column.to_f64();
        let last = get(&*targets, row, column); match last { TensorResult::Ok(number) => { if number.to_bits() != expected.to_bits() { return 100; } }, TensorResult::Err(problem) => { return 101; }, }
    }

    return 0;
}
fn passed() -> i32 { return 0; }
fn seek_defaults() -> bool {
    let path = b"tests/data/reader.oxds";
    for start in 0..5 {
        let opened = open_at(&path, 2, start);
        match opened {
            Result::Err(problem) => { return false; },
            Result::Ok(value) => {
                let mut reader = value; let mut seen = start;
                if position(&reader) != start || remaining(&reader) != 5 - start { return false; }
                while remaining(&reader) > 0 {
                    let loaded = next(&mut reader);
                    match loaded { Result::Err(problem) => { return false; }, Result::Ok(batch) => {
                        for row in 0..row_count(&batch.inputs) {
                            if verify_row(&batch.inputs, &batch.targets, row, seen) != 0 { return false; }
                            seen = seen + 1;
                        }
                        if position(&reader) != seen { return false; }
                    }, }
                }
                if seen != 5 { return false; }
            },
        }
    }
    let negative = open_at(&path, 2, -1);
    match negative { Result::Ok(reader) => { return false; }, Result::Err(error) => { if failure_code(error) != 7 { return false; } }, }
    let end = open_at(&path, 2, 5);
    match end { Result::Ok(reader) => { return false; }, Result::Err(error) => { if failure_code(error) != 7 { return false; } }, }
    return true;
}
fn main() -> i32 {
    if arg_count() == 1 && !seek_defaults() { return 109; }
    let mut expected_error = 0; let mut expected_rows = 5; let mut size = 2;
    if arg_count() != 1 { expected_error = argument(1); expected_rows = argument(2); size = argument(3); }
    if expected_error < 0 || expected_rows < 0 || size < 0 { return 90; }
    let path = b"tests/data/reader.oxds"; let opened = open(&path, size);
    match opened {
        Result::Err(problem) => { if failure_code(problem) == expected_error { return passed(); } return 91; },
        Result::Ok(value) => {
            let mut reader = value; let mut seen = 0; let mut batches = 0;
            while remaining(&reader) > 0 {
                let loaded = next(&mut reader);
                match loaded {
                    Result::Err(problem) => {
                        if failure_code(problem) != expected_error { return 92; }
                        if position(&reader) != seen { return 108; }
                        if arg_count() == 5 { let before = position(&reader); if !wait_for_inspection() || position(&reader) != before { return 106; } }
                        let poisoned = next(&mut reader); match poisoned { Result::Ok(batch) => { return 93; }, Result::Err(second) => { if failure_code(second) != 5 { return 94; } return passed(); }, }
                    },
                    Result::Ok(batch) => {
                        let height = row_count(&batch.inputs);
                        if height <= 0 || height > size || height != row_count(&batch.targets) || column_count(&batch.inputs) != feature_count(&reader) || column_count(&batch.targets) != target_count(&reader) { return 95; }
                        if expected_error == 0 {
                            for row in 0..height {
                                let checked = verify_row(&batch.inputs, &batch.targets, row, seen); if checked != 0 { return checked; }
                                seen = seen + 1;
                            }
                        } else { seen = seen + height; }
                        if position(&reader) != seen { return 102; } batches = batches + 1;
                    },
                }
            }
            if expected_error != 0 || seen != expected_rows || batches != (expected_rows - 1) / size + 1 { return 103; }
            if arg_count() == 5 { let before = position(&reader); if !wait_for_inspection() || position(&reader) != before { return 107; } }
            let ended = next(&mut reader); match ended { Result::Ok(batch) => { return 104; }, Result::Err(problem) => { if failure_code(problem) != 6 { return 105; } return passed(); }, }
        },
    }
}
