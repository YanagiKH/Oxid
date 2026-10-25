use crate::tensor::TensorResult;
use crate::tensor::Error;
use crate::tensor::model::Model;
use crate::tensor::optimizer::State;
use crate::tensor::model::session::Decoded;
use std::fs::OpenResult;
use std::fs::create;
use std::fs::open_read;

pub struct Recovery { pub model: Model, pub optimizer: State, pub slot: i32, pub legacy: bool }

pub fn save(path: &[u8], model: &Model, state: &State) -> TensorResult<()>{
    let encoded = crate::tensor::model::session::encode(&*model, &*state);
    match encoded {
        TensorResult::Ok(bytes) => {
            let opened = create(&*path);
            match opened {
                OpenResult::Opened(file) => { return crate::store(file, &bytes); },
                OpenResult::Error(code) => { return TensorResult::<()>:: Err(Error::IoError(code)); },
                OpenResult::AllocationFailed => { return TensorResult::<()>:: Err(Error::AllocationFailed); },
                OpenResult::MemoryLimit => { return TensorResult::<()>:: Err(Error::MemoryLimit); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); }
    }
}

pub fn load(path: &[u8]) -> TensorResult<Decoded>{
    let opened = open_read(&*path);
    match opened {
        OpenResult::Opened(file) => {
            let read = crate::read_all(file);
            match read {
                TensorResult::Ok(bytes) => { return crate::tensor::model::session::decode(&bytes); },
                TensorResult::Err(error) => { return TensorResult::<Decoded>:: Err(error); }
            }
        },
        OpenResult::Error(code) => { return TensorResult::<Decoded>:: Err(Error::IoError(code)); },
        OpenResult::AllocationFailed => { return TensorResult::<Decoded>:: Err(Error::AllocationFailed); },
        OpenResult::MemoryLimit => { return TensorResult::<Decoded>:: Err(Error::MemoryLimit); }
    }
}


fn selected(decoded: Decoded, slot: i32) -> TensorResult<Recovery>{
    let Decoded { model: model, optimizer: state, legacy: old_format } = decoded;
    return TensorResult::Ok(Recovery { model: model, optimizer: state, slot: slot, legacy: old_format });
}

fn second_only(path: &[u8]) -> TensorResult<Recovery>{
    let loaded = load(&*path);
    match loaded { TensorResult::Ok(decoded) => { return selected(decoded, 1); }, TensorResult::Err(error) => { return TensorResult::<Recovery>:: Err(error); } }
}

fn select(first: Decoded, second: TensorResult<Decoded>) -> TensorResult<Recovery>{
    match second {
        TensorResult::Ok(other) => {
            if crate::tensor::model::update_count(&other.model) > crate::tensor::model::update_count(&first.model) { return selected(other, 1); }
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
        TensorResult::Ok(decoded) => { return select(decoded, load(&*second_path)); },
        TensorResult::Err(error) => {
            let fallback = crate::recoverable(error);
            match fallback { TensorResult::Ok(done) => { return second_only(&*second_path); }, TensorResult::Err(fatal) => { return TensorResult::<Recovery>:: Err(fatal); } }
        }
    }
}
