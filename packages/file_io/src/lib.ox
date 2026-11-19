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
