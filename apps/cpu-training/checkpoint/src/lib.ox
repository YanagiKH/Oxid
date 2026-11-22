use crate::tensor::TensorResult;
use crate::tensor::Error;
use crate::tensor::model::Model;
use alloc::bytes::ByteBuf;
use std::fs::File;

pub mod training;
pub mod pipeline;

fn store(file: File, bytes: &[u8]) -> TensorResult<()> {
    return crate::storage::store(file, &*bytes);
}

fn read_all(file: File) -> TensorResult<ByteBuf> {
    return crate::storage::read_all(file);
}

pub fn save(path: &[u8], model: &Model) -> TensorResult<()>{
    let encoded = crate::tensor::model::checkpoint::encode(&*model);
    match encoded {
        TensorResult::Ok(bytes) => { return crate::storage::save_bytes(&*path, &bytes); },
        TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); }
    }
}

pub fn load(path: &[u8]) -> TensorResult<Model>{
    let read = crate::storage::load_bytes(&*path);
    match read {
        TensorResult::Ok(bytes) => { return crate::tensor::model::checkpoint::decode(&bytes); },
        TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); },
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

pub fn exists(path: &[u8]) -> TensorResult<bool> {
    return crate::storage::exists(&*path);
}

fn recoverable(error: Error) -> TensorResult<()> {
    return crate::storage::recoverable(error);
}
