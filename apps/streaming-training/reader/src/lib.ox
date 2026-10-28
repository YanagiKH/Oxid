use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use std::fs::File;
use std::fs::OpenResult;
use std::fs::FileStatus;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::close;
use std::fs::seek_absolute;
use std::io::ReadSomeStatus;
use crate::tensor::Tensor;
use crate::tensor::TensorResult;
use crate::tensor::Error as TensorError;
use crate::tensor::data::Batch;

pub enum Error { Tensor(TensorError), Io(i32), InvalidHeader, Truncated, TrailingData, InvalidBatchSize, Poisoned, End, InvalidStartRow }
pub enum Result<T> { Ok(T), Err(Error) }
pub struct Reader { file: File, scratch: ByteBuf, offset: i32, available: i32, rows: i32, features: i32, targets: i32, size: i32, cursor: i32, failed: bool }

fn allocate(size: i32) -> TensorResult<ByteBuf>{
    let mut bytes = new();
    let grown = try_resize(&mut bytes, size, 0u8);
    match grown {
        GrowStatus::Ok => { return TensorResult::Ok(bytes); },
        GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(TensorError::SizeOverflow); },
        GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(TensorError::MemoryLimit); },
        GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(TensorError::AllocationFailed); },
    }
}
fn read_u32(bytes: &[u8], start: i32) -> u32 {
    let mut number = 0u32;
    for offset in 0..4 { let byte = bytes[start + offset]; number = number | (byte.to_u32_checked() << (offset * 8)); }
    return number;
}
fn read_value(reader: &mut Reader) -> Result<f64>{
    let mut bytes = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let read = read_exact(&mut *reader, &mut bytes);
    match read { Result::Ok(done) => {}, Result::Err(error) => { return Result::<f64>:: Err(error); } }
    let mut bits = 0u64;
    for offset in 0..8 { let byte = bytes[offset]; bits = bits | (byte.to_u64_checked() << (offset * 8)); }
    if ((bits >> 52) & 2047u64) == 2047u64 { return Result::<f64>:: Err(Error::Tensor(TensorError::NonFinite)); }
    return Result::Ok(f64::from_bits(bits));
}
fn read_row(reader: &mut Reader, target: &mut Tensor, row: i32, columns: i32) -> Result<()>{
    for column in 0..columns {
        let read = read_value(&mut *reader);
        match read {
            Result::Ok(number) => {
                let stored = crate::tensor::set(&mut *target, row, column, number);
                match stored { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return Result::<()>:: Err(Error::Tensor(error)); } }
            },
            Result::Err(error) => { return Result::<()>:: Err(error); }
        }
    }
    return Result::Ok(());
}

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
    let made = allocate(4096);
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
                    let rows = read_u32(&header, 8);
                    let features = read_u32(&header, 12);
                    let targets = read_u32(&header, 16);
                    let reserved = read_u32(&header, 20);
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

pub fn open_at(path: &[u8], size: i32, start_row: i32) -> Result<Reader>{
    if size <= 0 { return Result::<Reader>:: Err(Error::InvalidBatchSize); }
    if start_row < 0 { return Result::<Reader>:: Err(Error::InvalidStartRow); }
    // Keep ordinary sequential opening, including nonseekable inputs, intact.
    if start_row == 0 { return open(&*path, size); }
    let opened = open(&*path, size);
    match opened {
        Result::Err(error) => { return Result::<Reader>:: Err(error); },
        Result::Ok(value) => {
            let mut reader = value;
            if start_row >= reader.rows { return reject::<Reader>(&mut reader, Error::InvalidStartRow); }
            let columns = reader.features + reader.targets;
            let wide_columns = columns.to_i64_checked();
            let wide_start = start_row.to_i64_checked();
            // Validated dimensions fit this signed 64-bit byte offset. The
            // language's checked arithmetic also rejects any future overflow.
            let byte_offset = 24i64 + wide_start * wide_columns * 8i64;
            let moved = seek_absolute(&mut *reader.file, byte_offset);
            match moved {
                FileStatus::Error(code) => { return reject::<Reader>(&mut reader, Error::Io(code)); },
                FileStatus::Ok => {
                    // open() may have prefetched beyond the header. Only a
                    // successful absolute seek replaces that buffered view.
                    reader.offset = 0;
                    reader.available = 0;
                    reader.cursor = start_row;
                    return Result::Ok(reader);
                }
            }
        }
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

pub fn next(reader: &mut Reader) -> Result<Batch>{
    if reader.failed { return Result::<Batch>:: Err(Error::Poisoned); }
    if reader.cursor == reader.rows { return Result::<Batch>:: Err(Error::End); }
    reader.failed = true;
    let mut count = reader.rows - reader.cursor;
    if count > reader.size { count = reader.size; }
    let input = crate::tensor::filled(count, reader.features, 0.0);
    match input {
        TensorResult::Ok(inputs) => {
            let mut first = inputs;
            let output = crate::tensor::filled(count, reader.targets, 0.0);
            match output {
                TensorResult::Ok(targets) => {
                    let mut second = targets;
                    let mut row = 0;
                    while row < count {
                        let features = reader.features;
                        let target_columns = reader.targets;
                        let read = read_row(&mut *reader, &mut first, row, features);
                        match read { Result::Ok(done) => {}, Result::Err(error) => { return reject::<Batch>(&mut *reader, error); } }
                        let target_read = read_row(&mut *reader, &mut second, row, target_columns);
                        match target_read { Result::Ok(done) => {}, Result::Err(error) => { return reject::<Batch>(&mut *reader, error); } }
                        row = row + 1;
                    }
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
