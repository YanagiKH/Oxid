use alloc::bytes::ByteBuf;
use crate::tensor::TensorResult;
use crate::tensor::Error;
use crate::tensor::model::Model;
use crate::tensor::optimizer::State;
use crate::tensor::data::Dataset;
use crate::tensor::data::Sampler;
use crate::tensor::data::session::Restored;
use std::fs::OpenResult;
use std::fs::create;
use std::fs::open_read;

pub struct Recovery { pub model: Model, pub optimizer: State, pub dataset: Dataset, pub sampler: Sampler, pub slot: i32 }

fn store(path: &[u8], bytes: &[u8]) -> TensorResult<()>{
    let opened = create(&*path);
    match opened {
        OpenResult::Opened(file) => { return crate::store(file, &*bytes); },
        OpenResult::Error(code) => { return TensorResult::<()>:: Err(Error::IoError(code)); },
        OpenResult::AllocationFailed => { return TensorResult::<()>:: Err(Error::AllocationFailed); },
        OpenResult::MemoryLimit => { return TensorResult::<()>:: Err(Error::MemoryLimit); }
    }
}

fn read(path: &[u8]) -> TensorResult<ByteBuf>{
    let opened = open_read(&*path);
    match opened {
        OpenResult::Opened(file) => { return crate::read_all(file); },
        OpenResult::Error(code) => { return TensorResult::<ByteBuf>:: Err(Error::IoError(code)); },
        OpenResult::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); },
        OpenResult::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); }
    }
}

pub fn save_dataset(path: &[u8], data: &Dataset) -> TensorResult<()>{
    let encoded = crate::tensor::data::checkpoint::encode(&*data);
    match encoded { TensorResult::Ok(bytes) => { return store(&*path, &bytes); }, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
}

pub fn load_dataset(path: &[u8]) -> TensorResult<Dataset>{
    let loaded = read(&*path);
    match loaded { TensorResult::Ok(bytes) => { return crate::tensor::data::checkpoint::decode(&bytes); }, TensorResult::Err(error) => { return TensorResult::<Dataset>:: Err(error); } }
}

pub fn save(path: &[u8], model: &Model, state: &State, data: &Dataset, sampler: &Sampler) -> TensorResult<()>{
    let encoded = crate::tensor::data::session::encode(&*model, &*state, &*data, &*sampler);
    match encoded { TensorResult::Ok(bytes) => { return store(&*path, &bytes); }, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
}

pub fn load(path: &[u8]) -> TensorResult<Restored>{
    let loaded = read(&*path);
    match loaded { TensorResult::Ok(bytes) => { return crate::tensor::data::session::decode(&bytes); }, TensorResult::Err(error) => { return TensorResult::<Restored>:: Err(error); } }
}

fn selected(restored: Restored, slot: i32) -> TensorResult<Recovery>{
    let Restored { model: model, optimizer: optimizer, dataset: dataset, sampler: sampler } = restored;
    return TensorResult::Ok(Recovery { model: model, optimizer: optimizer, dataset: dataset, sampler: sampler, slot: slot });
}

fn second_only(path: &[u8]) -> TensorResult<Recovery>{
    let loaded = load(&*path);
    match loaded { TensorResult::Ok(restored) => { return selected(restored, 1); }, TensorResult::Err(error) => { return TensorResult::<Recovery>:: Err(error); } }
}

fn select(first: Restored, second: TensorResult<Restored>) -> TensorResult<Recovery>{
    match second {
        TensorResult::Ok(other) => {
            if crate::tensor::data::batch_count(&other.sampler) > crate::tensor::data::batch_count(&first.sampler) { return selected(other, 1); }
            return selected(first, 0);
        },
        TensorResult::Err(error) => {
            let fallback = crate::recoverable(error);
            match fallback { TensorResult::Ok(done) => { return selected(first, 0); }, TensorResult::Err(fatal) => { return TensorResult::<Recovery>:: Err(fatal); } }
        }
    }
}

pub fn load_latest(first_path: &[u8], second_path: &[u8]) -> TensorResult<Recovery>{
    let first = load(&*first_path);
    match first {
        TensorResult::Ok(restored) => { return select(restored, load(&*second_path)); },
        TensorResult::Err(error) => {
            let fallback = crate::recoverable(error);
            match fallback { TensorResult::Ok(done) => { return second_only(&*second_path); }, TensorResult::Err(fatal) => { return TensorResult::<Recovery>:: Err(fatal); } }
        }
    }
}
