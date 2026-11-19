use std::fs::File;
use std::fs::read_some;
use std::fs::write_some;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;

pub struct TransferError {
    pub transferred: i32,
    pub code: i32,
}

pub enum ReadExact {
    Complete,
    EndOfFile(i32),
    Error(TransferError),
}

pub enum WriteAll {
    Complete,
    NoProgress(i32),
    Error(TransferError),
}

pub fn read_exact(file: &mut File, bytes: &mut [u8]) -> ReadExact {
    let mut offset = 0;
    let length = bytes.len();
    while offset < length {
        let read = read_some(&mut *file, &mut *bytes[offset..length]);
        match read {
            ReadSomeStatus::Data(count) => { offset = offset + count; },
            ReadSomeStatus::Eof => { return ReadExact::EndOfFile(offset); },
            ReadSomeStatus::Error(code) => {
                return ReadExact::Error(TransferError { transferred: offset, code: code });
            },
        }
    }
    return ReadExact::Complete;
}

pub fn write_all(file: &mut File, bytes: &[u8]) -> WriteAll {
    let mut offset = 0;
    let length = bytes.len();
    while offset < length {
        let written = write_some(&mut *file, &*bytes[offset..length]);
        match written {
            WriteSomeStatus::Written(count) => {
                if count == 0 { return WriteAll::NoProgress(offset); }
                offset = offset + count;
            },
            WriteSomeStatus::Error(code) => {
                return WriteAll::Error(TransferError { transferred: offset, code: code });
            },
        }
    }
    return WriteAll::Complete;
}

pub struct CopyError {
    pub written: i64,
    pub code: i32,
}

pub enum CopyResult {
    Complete(i64),
    EmptyBuffer,
    TooLarge(i64),
    ReadError(CopyError),
    WriteError(CopyError),
    NoProgress(i64),
}

pub fn copy(input: &mut File, output: &mut File, buffer: &mut [u8]) -> CopyResult {
    if buffer.len() == 0 { return CopyResult::EmptyBuffer; }
    let mut total = 0i64;
    while true {
        let read = read_some(&mut *input, &mut *buffer);
        match read {
            ReadSomeStatus::Eof => { return CopyResult::Complete(total); },
            ReadSomeStatus::Error(code) => {
                return CopyResult::ReadError(CopyError { written: total, code: code });
            },
            ReadSomeStatus::Data(count) => {
                let received = count.to_i64();
                if total > 9223372036854775807i64 - received {
                    return CopyResult::TooLarge(total);
                }
                let write = write_all(&mut *output, &*buffer[0..count]);
                match write {
                    WriteAll::Complete => { total = total + received; },
                    WriteAll::NoProgress(partial) => {
                        return CopyResult::NoProgress(total + partial.to_i64());
                    },
                    WriteAll::Error(error) => {
                        let partial = error.transferred;
                        let written = total + partial.to_i64();
                        return CopyResult::WriteError(CopyError { written: written, code: error.code });
                    },
                }
            },
        }
    }
    return CopyResult::NoProgress(total);
}
