use alloc::bytes::ByteBuf;
use std::fs::File;
use std::fs::OpenResult;
use std::fs::FileStatus;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::close;
use std::io::ReadSomeStatus;
use crate::Tensor;
use crate::TensorResult;
use crate::Error as TensorError;
use crate::data::Batch;

pub enum Error { Tensor(TensorError), Io(i32), InvalidHeader, Truncated, TrailingData, InvalidBatchSize, Poisoned, End }
pub enum Result<T> { Ok(T), Err(Error) }
pub struct Reader { file: File, scratch: ByteBuf, offset: i32, available: i32, rows: i32, features: i32, targets: i32, size: i32, cursor: i32, failed: bool }

fn reject<T>(reader: &mut Reader, error: Error) -> Result<T>{
    reader.failed = true;
    let closed = close(&mut *reader.file);
    match closed { FileStatus::Ok => {}, FileStatus::Error(code) => {} }
    return Result::<T>:: Err(error);
}

fn buffer_length(bytes: &[u8]) -> i32 { return bytes.len(); }
fn copy_bytes(source: &[u8], target: &mut [u8]) -> () { for index in 0..source.len() { target[index] = source[index]; } return; }

fn refill(reader: &mut Reader) -> Result<bool>{
    while true {
        let read = read_some(&mut *reader.file, &mut *reader.scratch);
        match read {
            ReadSomeStatus::Data(count) => {
                if count <= 0 || count > buffer_length(&*reader.scratch) { return Result::<bool>:: Err(Error::Io(5)); }
                reader.offset = 0;
                reader.available = count;
                return Result::Ok(true);
            },
            ReadSomeStatus::Eof => { reader.offset = 0; reader.available = 0; return Result::Ok(false); },
            ReadSomeStatus::Error(code) => { if code != 4 { return Result::<bool>:: Err(Error::Io(code)); } }
        }
    }
    return Result::<bool>:: Err(Error::Io(5));
}

fn read_exact(reader: &mut Reader, target: &mut [u8]) -> Result<()>{
    let mut written = 0;
    while written < target.len() {
        if reader.offset == reader.available {
            let read = refill(&mut *reader);
            match read { Result::Ok(more) => { if !more { return Result::<()>:: Err(Error::Truncated); } }, Result::Err(error) => { return Result::<()>:: Err(error); } }
        }
        let mut count = reader.available - reader.offset;
        let needed = target.len() - written;
        if count > needed { count = needed; }
        copy_bytes(&*reader.scratch[reader.offset..reader.offset + count], &mut *target[written..written + count]);
        reader.offset = reader.offset + count;
        written = written + count;
    }
    return Result::Ok(());
}

pub fn open(path: &[u8], size: i32) -> Result<Reader>{
    if size <= 0 { return Result::<Reader>:: Err(Error::InvalidBatchSize); }
    let made = crate::data::allocate(4096);
    match made {
        TensorResult::Ok(buffer) => {
            let opened = open_read(&*path);
            match opened {
                OpenResult::Opened(file) => {
                    let mut reader = Reader { file: file, scratch: buffer, offset: 0, available: 0, rows: 0, features: 0, targets: 0, size: size, cursor: 0, failed: false };
                    let mut header = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
                    let read = read_exact(&mut reader, &mut header);
                    match read { Result::Ok(done) => {}, Result::Err(error) => { return reject::<Reader>(&mut reader, error); } }
                    let magic = b"OXDS0001";
                    for index in 0..8 { if header[index] != magic[index] { return reject::<Reader>(&mut reader, Error::InvalidHeader); } }
                    let rows = crate::codec::get32(&header, 8);
                    let features = crate::codec::get32(&header, 12);
                    let targets = crate::codec::get32(&header, 16);
                    let reserved = crate::codec::get32(&header, 20);
                    if rows == 0u32 || rows > 2147483647u32 || features == 0u32 || features > 268435455u32 || targets == 0u32 || targets > 268435455u32 || reserved != 0u32 { return reject::<Reader>(&mut reader, Error::InvalidHeader); }
                    if features + targets > 268435455u32 { return reject::<Reader>(&mut reader, Error::InvalidHeader); }
                    reader.rows = rows.to_i32_checked();
                    reader.features = features.to_i32_checked();
                    reader.targets = targets.to_i32_checked();
                    return Result::Ok(reader);
                },
                OpenResult::Error(code) => { return Result::<Reader>:: Err(Error::Io(code)); },
                OpenResult::MemoryLimit => { return Result::<Reader>:: Err(Error::Tensor(TensorError::MemoryLimit)); },
                OpenResult::AllocationFailed => { return Result::<Reader>:: Err(Error::Tensor(TensorError::AllocationFailed)); }
            }
        },
        TensorResult::Err(error) => { return Result::<Reader>:: Err(Error::Tensor(error)); }
    }
}

pub fn row_count(reader: &Reader) -> i32 { return reader.rows; }
pub fn feature_count(reader: &Reader) -> i32 { return reader.features; }
pub fn target_count(reader: &Reader) -> i32 { return reader.targets; }
pub fn position(reader: &Reader) -> i32 { return reader.cursor; }
pub fn remaining(reader: &Reader) -> i32 { return reader.rows - reader.cursor; }

fn finish(reader: &mut Reader) -> Result<()>{
    if reader.offset < reader.available { return Result::<()>:: Err(Error::TrailingData); }
    let read = refill(&mut *reader);
    match read { Result::Ok(more) => { if more { return Result::<()>:: Err(Error::TrailingData); } }, Result::Err(error) => { return Result::<()>:: Err(error); } }
    let closed = close(&mut *reader.file);
    match closed { FileStatus::Ok => { return Result::Ok(()); }, FileStatus::Error(code) => { return Result::<()>:: Err(Error::Io(code)); } }
}

fn finite_tensor(value: &Tensor) -> bool {
    for index in 0..value.rows * value.columns {
        let number = crate::codec::load(&*value.data, index);
        if ((number.to_bits() >> 52) & 2047u64) == 2047u64 { return false; }
    }
    return true;
}

pub fn next(reader: &mut Reader) -> Result<Batch>{
    if reader.failed { return Result::<Batch>:: Err(Error::Poisoned); }
    if reader.cursor == reader.rows { return Result::<Batch>:: Err(Error::End); }
    reader.failed = true;
    let mut count = reader.rows - reader.cursor;
    if count > reader.size { count = reader.size; }
    let input = crate::filled(count, reader.features, 0.0);
    match input {
        TensorResult::Ok(inputs) => {
            let mut first = inputs;
            let output = crate::filled(count, reader.targets, 0.0);
            match output {
                TensorResult::Ok(targets) => {
                    let mut second = targets;
                    let mut row = 0;
                    while row < count {
                        let input_start = row * reader.features * 8;
                        let input_end = input_start + reader.features * 8;
                        let read = read_exact(&mut *reader, &mut first.data[input_start..input_end]);
                        match read { Result::Ok(done) => {}, Result::Err(error) => { return reject::<Batch>(&mut *reader, error); } }
                        let target_start = row * reader.targets * 8;
                        let target_end = target_start + reader.targets * 8;
                        let target_read = read_exact(&mut *reader, &mut second.data[target_start..target_end]);
                        match target_read { Result::Ok(done) => {}, Result::Err(error) => { return reject::<Batch>(&mut *reader, error); } }
                        row = row + 1;
                    }
                    if !finite_tensor(&first) || !finite_tensor(&second) { return reject::<Batch>(&mut *reader, Error::Tensor(TensorError::NonFinite)); }
                    if reader.cursor + count == reader.rows {
                        let ended = finish(&mut *reader);
                        match ended { Result::Ok(done) => {}, Result::Err(error) => { return reject::<Batch>(&mut *reader, error); } }
                    }
                    reader.cursor = reader.cursor + count;
                    reader.failed = false;
                    return Result::Ok(Batch { inputs: first, targets: second });
                },
                TensorResult::Err(error) => { return reject::<Batch>(&mut *reader, Error::Tensor(error)); }
            }
        },
        TensorResult::Err(error) => { return reject::<Batch>(&mut *reader, Error::Tensor(error)); }
    }
}
