use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use crate::TensorResult;
use crate::Error;
use crate::model::Model;
use crate::optimizer::State;

pub struct Decoded { pub model: Model, pub optimizer: State, pub legacy: bool }

fn copy(target: &mut [u8], start: i32, source: &[u8]) -> () {
    for index in 0..source.len() { target[start + index] = source[index]; }
    return;
}

fn matching(model: &Model, state: &State) -> bool {
    let elements = crate::model::parameter_elements(&*model);
    return elements >= 0 && crate::optimizer::row_count(&*state) == 1 && crate::optimizer::column_count(&*state) == elements;
}

fn combine(model: &[u8], state: &[u8]) -> TensorResult<ByteBuf>{
    if model.len() > 16777188 || state.len() > 16777188 - model.len() { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }
    let end = 24 + model.len() + state.len();
    let mut output = new();
    let grown = try_resize(&mut output, end + 4, 0u8);
    match grown { GrowStatus::Ok => {}, GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }, GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); }, GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); } }
    let magic = b"OXTR0001";
    copy(&mut output, 0, &magic);
    let model_length = model.len();
    let state_length = state.len();
    crate::codec::put64(&mut output, 8, model_length.to_u64_checked());
    crate::codec::put64(&mut output, 16, state_length.to_u64_checked());
    copy(&mut output, 24, &*model);
    copy(&mut output, 24 + model.len(), &*state);
    let crc = crate::codec::checksum(&output, end);
    crate::codec::put32(&mut output, end, crc);
    return TensorResult::Ok(output);
}

pub fn encode(model: &Model, state: &State) -> TensorResult<ByteBuf>{
    if !matching(&*model, &*state) { return TensorResult::<ByteBuf>:: Err(Error::ShapeMismatch); }
    let encoded_model = crate::model::checkpoint::encode(&*model);
    match encoded_model {
        TensorResult::Ok(model_bytes) => {
            let encoded_state = crate::optimizer::checkpoint::encode(&*state);
            match encoded_state {
                TensorResult::Ok(state_bytes) => { return combine(&model_bytes, &state_bytes); },
                TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
    }
}

fn is_magic(bytes: &[u8], magic: &[u8]) -> bool {
    if bytes.len() < magic.len() { return false; }
    for index in 0..magic.len() { if bytes[index] != magic[index] { return false; } }
    return true;
}

fn legacy(bytes: &[u8]) -> TensorResult<Decoded>{
    let decoded_model = crate::model::checkpoint::decode(&*bytes);
    match decoded_model {
        TensorResult::Ok(model) => {
            let elements = crate::model::parameter_elements(&model);
            if elements < 0 { return TensorResult::<Decoded>:: Err(Error::InvalidCheckpoint); }
            let initialized = crate::optimizer::initialize_sgd(1, elements, 0.05, crate::model::update_count(&model));
            match initialized {
                TensorResult::Ok(state) => { return TensorResult::Ok(Decoded { model: model, optimizer: state, legacy: true }); },
                TensorResult::Err(error) => { return TensorResult::<Decoded>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Decoded>:: Err(error); }
    }
}

pub fn decode(bytes: &[u8]) -> TensorResult<Decoded>{
    let old_magic = b"OXMD0001";
    if is_magic(&*bytes, &old_magic) { return legacy(&*bytes); }
    let magic = b"OXTR0001";
    if bytes.len() < 28 || bytes.len() > 16777216 || !is_magic(&*bytes, &magic) { return TensorResult::<Decoded>:: Err(Error::InvalidCheckpoint); }
    let model_length = crate::codec::get64(&*bytes, 8);
    let state_length = crate::codec::get64(&*bytes, 16);
    let available = bytes.len() - 28;
    let available64 = available.to_u64_checked();
    if model_length > available64 || state_length > available64 || model_length != available64 - state_length { return TensorResult::<Decoded>:: Err(Error::InvalidCheckpoint); }
    let end = bytes.len() - 4;
    if crate::codec::get32(&*bytes, end) != crate::codec::checksum(&*bytes, end) { return TensorResult::<Decoded>:: Err(Error::InvalidCheckpoint); }
    let split = 24 + model_length.to_i32_checked();
    let decoded_model = crate::model::checkpoint::decode(&*bytes[24..split]);
    match decoded_model {
        TensorResult::Ok(model) => {
            let decoded_state = crate::optimizer::checkpoint::decode(&*bytes[split..end]);
            match decoded_state {
                TensorResult::Ok(state) => {
                    if !matching(&model, &state) { return TensorResult::<Decoded>:: Err(Error::InvalidCheckpoint); }
                    return TensorResult::Ok(Decoded { model: model, optimizer: state, legacy: false });
                },
                TensorResult::Err(error) => { return TensorResult::<Decoded>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Decoded>:: Err(error); }
    }
}
