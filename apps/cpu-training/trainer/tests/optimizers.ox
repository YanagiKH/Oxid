use crate::tensor::TensorResult;
use crate::tensor::Tensor;
use crate::tensor::optimizer::State;
use crate::tensor::model::Model;
use crate::tensor::model::session::Decoded;

fn status(result: TensorResult<()>) -> bool {
    match result { TensorResult::Ok(done) => { return true; }, TensorResult::Err(error) => { return false; } }
}

fn same(first: &[u8], second: &[u8]) -> bool {
    if first.len() != second.len() { return false; }
    for index in 0..first.len() { if first[index] != second[index] { return false; } }
    return true;
}

fn reencoded(state: State, original: &[u8]) -> i32 {
    if crate::tensor::optimizer::step_count(&state) != 1u64 { return 1; }
    let saved = crate::tensor::optimizer::checkpoint::encode(&state);
    match saved { TensorResult::Ok(bytes) => { if !same(&bytes, &*original) { return 2; } return 0; }, TensorResult::Err(error) => { return 3; } }
}

fn roundtrip(state: &State) -> i32 {
    let saved = crate::tensor::optimizer::checkpoint::encode(&*state);
    match saved {
        TensorResult::Ok(bytes) => {
            let loaded = crate::tensor::optimizer::checkpoint::decode(&bytes);
            match loaded { TensorResult::Ok(restored) => { return reencoded(restored, &bytes); }, TensorResult::Err(error) => { return 4; } }
        },
        TensorResult::Err(error) => { return 5; }
    }
}

fn session_checked(bytes: &[u8], legacy: bool, steps: u64) -> i32 {
    let loaded = crate::tensor::model::session::decode(&*bytes);
    match loaded {
        TensorResult::Ok(decoded) => {
            let Decoded { model: model, optimizer: state, legacy: old_format } = decoded;
            if old_format != legacy || crate::tensor::model::update_count(&model) != 0u64 { return 6; }
            if crate::tensor::optimizer::step_count(&state) != steps { return 7; }
            if crate::tensor::optimizer::row_count(&state) != 1 || crate::tensor::optimizer::column_count(&state) != 33 { return 8; }
            return 0;
        },
        TensorResult::Err(error) => { return 9; }
    }
}

fn sessions(model: &Model, state: &State) -> i32 {
    let saved = crate::tensor::model::session::encode(&*model, &*state);
    match saved { TensorResult::Ok(bytes) => { let tested = session_checked(&bytes, false, 1u64); if tested != 0 { return tested; } }, TensorResult::Err(error) => { return 10; } }
    let old = crate::tensor::model::checkpoint::encode(&*model);
    match old { TensorResult::Ok(bytes) => { return session_checked(&bytes, true, 0u64); }, TensorResult::Err(error) => { return 11; } }
}

fn apply(model: &Model, created: State, target: Tensor, gradient: &Tensor) -> i32 {
    let mut state = created;
    let mut parameter = target;
    if !status(crate::tensor::optimizer::step(&mut state, &mut parameter, &*gradient)) { return 12; }
    let tested = roundtrip(&state); if tested != 0 { return tested; }
    return sessions(&*model, &state);
}

fn initialized(model: &Model, gradient: &Tensor, target: Tensor) -> i32 {
    let selected = crate::tensor::optimizer::adam(0.01, 0.9, 0.999, 1e-8);
    match selected {
        TensorResult::Ok(config) => {
            let made = crate::tensor::optimizer::initialize(1, 33, config);
            match made { TensorResult::Ok(state) => { return apply(&*model, state, target, &*gradient); }, TensorResult::Err(error) => { return 13; } }
        },
        TensorResult::Err(error) => { return 14; }
    }
}

fn parameters(model: &Model) -> i32 {
    let copied = crate::tensor::model::parameters(&*model);
    match copied {
        TensorResult::Ok(target) => {
            let made = crate::tensor::filled(1, 33, 0.0);
            match made { TensorResult::Ok(gradient) => { return initialized(&*model, &gradient, target); }, TensorResult::Err(error) => { return 15; } }
        },
        TensorResult::Err(error) => { return 16; }
    }
}

fn main() -> i32 {
    let made = crate::tensor::model::initialize(2, 8, 1, 42u32);
    match made { TensorResult::Ok(model) => { return parameters(&model); }, TensorResult::Err(error) => { return 17; } }
}
