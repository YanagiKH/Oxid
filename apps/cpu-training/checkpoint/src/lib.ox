use crate::tensor::TensorResult;
use crate::tensor::Error;
use crate::tensor::model::Model;
use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use std::fs::File;
use std::fs::OpenResult;
use std::fs::FileStatus;
use std::fs::create;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::write_some;
use std::fs::sync_all;
use std::fs::close;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
pub mod training;

fn write_all(file: &mut File, bytes: &[u8]) -> TensorResult<()>{
    let mut offset = 0;
    while offset < bytes.len() {
        let written = write_some(&mut *file, &*bytes[offset..bytes.len()]);
        match written {
            WriteSomeStatus::Written(count) => {
                if count <= 0 { return TensorResult::<()>:: Err(Error::WriteZero); }
                offset = offset + count;
            },
            WriteSomeStatus::Error(code) => { if code != 4 { return TensorResult::<()>:: Err(Error::IoError(code)); } }
        }
    }
    return TensorResult::Ok(());
}

fn store(file: File, bytes: &[u8]) -> TensorResult<()>{
    let mut output = file;
    let written = write_all(&mut output, &*bytes);
    match written { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let synced = sync_all(&mut output);
    match synced { FileStatus::Ok => {}, FileStatus::Error(code) => { return TensorResult::<()>:: Err(Error::IoError(code)); } }
    let closed = close(&mut output);
    match closed { FileStatus::Ok => {}, FileStatus::Error(code) => { return TensorResult::<()>:: Err(Error::IoError(code)); } }
    return TensorResult::Ok(());
}

pub fn save(path: &[u8], model: &Model) -> TensorResult<()>{
    let encoded = crate::tensor::model::checkpoint::encode(&*model);
    match encoded {
        TensorResult::Ok(bytes) => {
            let opened = create(&*path);
            match opened {
                OpenResult::Opened(file) => { return store(file, &bytes); },
                OpenResult::Error(code) => { return TensorResult::<()>:: Err(Error::IoError(code)); },
                OpenResult::AllocationFailed => { return TensorResult::<()>:: Err(Error::AllocationFailed); },
                OpenResult::MemoryLimit => { return TensorResult::<()>:: Err(Error::MemoryLimit); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); }
    }
}

fn append(bytes: &mut ByteBuf, chunk: &[u8]) -> TensorResult<()>{
    let grown = try_extend(&mut *bytes, &*chunk);
    match grown {
        GrowStatus::Ok => { return TensorResult::Ok(()); },
        GrowStatus::CapacityOverflow => { return TensorResult::<()>:: Err(Error::SizeOverflow); },
        GrowStatus::AllocationFailed => { return TensorResult::<()>:: Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return TensorResult::<()>:: Err(Error::MemoryLimit); }
    }
}

fn oversized(bytes: &[u8], count: i32) -> bool {
    return count > 16777216 || bytes.len() > 16777216 - count;
}

fn read_all(file: File) -> TensorResult<ByteBuf>{
    let mut input = file;
    let mut bytes = new();
    let mut chunk = new();
    let grown = try_resize(&mut chunk, 1024, 0u8);
    match grown {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); },
        GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); }
    }
    while true {
        let read = read_some(&mut input, &mut chunk);
        match read {
            ReadSomeStatus::Data(count) => {
                if count <= 0 { return TensorResult::<ByteBuf>:: Err(Error::IoError(5)); }
                if oversized(&bytes, count) { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); }
                let appended = append(&mut bytes, &chunk[0..count]);
                match appended { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); } }
            },
            ReadSomeStatus::Eof => {
                let closed = close(&mut input);
                match closed { FileStatus::Ok => {}, FileStatus::Error(code) => { return TensorResult::<ByteBuf>:: Err(Error::IoError(code)); } }
                return TensorResult::Ok(bytes);
            },
            ReadSomeStatus::Error(code) => { if code != 4 { return TensorResult::<ByteBuf>:: Err(Error::IoError(code)); } }
        }
    }
    return TensorResult::<ByteBuf>:: Err(Error::IoError(5));
}

pub fn load(path: &[u8]) -> TensorResult<Model>{
    let opened = open_read(&*path);
    match opened {
        OpenResult::Opened(file) => {
            let read = read_all(file);
            match read {
                TensorResult::Ok(bytes) => { return crate::tensor::model::checkpoint::decode(&bytes); },
                TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); }
            }
        },
        OpenResult::Error(code) => { return TensorResult::<Model>:: Err(Error::IoError(code)); },
        OpenResult::AllocationFailed => { return TensorResult::<Model>:: Err(Error::AllocationFailed); },
        OpenResult::MemoryLimit => { return TensorResult::<Model>:: Err(Error::MemoryLimit); }
    }
}

pub struct Recovery { pub model: Model, pub slot: i32 }

fn recover_first(model: Model, error: Error) -> TensorResult<Recovery>{
    let checked = recoverable(error);
    match checked { TensorResult::Ok(done) => { return TensorResult::Ok(Recovery { model: model, slot: 0 }); }, TensorResult::Err(fatal) => { return TensorResult::<Recovery>:: Err(fatal); } }
}

fn second_only(path: &[u8]) -> TensorResult<Recovery>{
    let loaded = load(&*path);
    match loaded { TensorResult::Ok(model) => { return TensorResult::Ok(Recovery { model: model, slot: 1 }); }, TensorResult::Err(error) => { return TensorResult::<Recovery>:: Err(error); } }
}

fn recover_second(error: Error, path: &[u8]) -> TensorResult<Recovery>{
    let checked = recoverable(error);
    match checked { TensorResult::Ok(done) => { return second_only(&*path); }, TensorResult::Err(fatal) => { return TensorResult::<Recovery>:: Err(fatal); } }
}

fn select_second(first: Model, second: TensorResult<Model>) -> TensorResult<Recovery>{
    match second {
        TensorResult::Ok(other) => {
            if crate::tensor::model::update_count(&other) > crate::tensor::model::update_count(&first) { return TensorResult::Ok(Recovery { model: other, slot: 1 }); }
            return TensorResult::Ok(Recovery { model: first, slot: 0 });
        },
        TensorResult::Err(error) => {
            return recover_first(first, error);
        }
    }
}

pub fn load_latest_with_slot(first_path: &[u8], second_path: &[u8]) -> TensorResult<Recovery>{
    let first = load(&*first_path);
    match first {
        TensorResult::Ok(model) => {
            let second = load(&*second_path);
            return select_second(model, second);
        },
        TensorResult::Err(error) => { return recover_second(error, &*second_path); }
    }
}

pub fn load_latest(first_path: &[u8], second_path: &[u8]) -> TensorResult<Model>{
    let loaded = load_latest_with_slot(&*first_path, &*second_path);
    match loaded {
        TensorResult::Ok(recovery) => { let Recovery { model: selected, slot: unused_slot } = recovery; return TensorResult::Ok(selected); },
        TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); }
    }
}

pub fn exists(path: &[u8]) -> TensorResult<bool>{
    let opened = open_read(&*path);
    match opened {
        OpenResult::Opened(file) => {
            let mut input = file;
            let closed = close(&mut input);
            match closed { FileStatus::Ok => { return TensorResult::Ok(true); }, FileStatus::Error(code) => { return TensorResult::<bool>:: Err(Error::IoError(code)); } }
        },
        OpenResult::Error(code) => { if code == 2 { return TensorResult::Ok(false); } return TensorResult::<bool>:: Err(Error::IoError(code)); },
        OpenResult::AllocationFailed => { return TensorResult::<bool>:: Err(Error::AllocationFailed); },
        OpenResult::MemoryLimit => { return TensorResult::<bool>:: Err(Error::MemoryLimit); }
    }
}

fn recoverable(error: Error) -> TensorResult<()>{
    match error {
        Error::IoError(code) => { if code == 2 { return TensorResult::Ok(()); } return TensorResult::<()>:: Err(Error::IoError(code)); },
        Error::InvalidCheckpoint => { return TensorResult::Ok(()); },
        Error::NegativeShape => { return TensorResult::<()>:: Err(Error::NegativeShape); },
        Error::SizeOverflow => { return TensorResult::<()>:: Err(Error::SizeOverflow); },
        Error::ShapeMismatch => { return TensorResult::<()>:: Err(Error::ShapeMismatch); },
        Error::Bounds => { return TensorResult::<()>:: Err(Error::Bounds); },
        Error::AllocationFailed => { return TensorResult::<()>:: Err(Error::AllocationFailed); },
        Error::MemoryLimit => { return TensorResult::<()>:: Err(Error::MemoryLimit); },
        Error::InvalidStorage => { return TensorResult::<()>:: Err(Error::InvalidStorage); },
        Error::EmptyReduction => { return TensorResult::<()>:: Err(Error::EmptyReduction); },
        Error::InvalidNode => { return TensorResult::<()>:: Err(Error::InvalidNode); },
        Error::GraphCapacity => { return TensorResult::<()>:: Err(Error::GraphCapacity); },
        Error::NonFinite => { return TensorResult::<()>:: Err(Error::NonFinite); },
        Error::InvalidRate => { return TensorResult::<()>:: Err(Error::InvalidRate); },
        Error::InvalidOptimizer => { return TensorResult::<()>:: Err(Error::InvalidOptimizer); },
        Error::StepOverflow => { return TensorResult::<()>:: Err(Error::StepOverflow); },
        Error::WriteZero => { return TensorResult::<()>:: Err(Error::WriteZero); }
    }
}
