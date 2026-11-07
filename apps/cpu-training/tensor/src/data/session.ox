use alloc::bytes::ByteBuf;
use crate::TensorResult;
use crate::Error;
use crate::model::Model;
use crate::optimizer::State;
use crate::data::Dataset;
use crate::data::Sampler;
use crate::model::session::Decoded;

pub struct Restored { pub model: Model, pub optimizer: State, pub dataset: Dataset, pub sampler: Sampler }

fn matching(model: &Model, data: &Dataset, sampler: &Sampler) -> bool {
    return crate::data::valid(&*sampler) && sampler.rows == crate::data::row_count(&*data) && crate::model::input_count(&*model) == crate::data::feature_count(&*data) && crate::model::output_count(&*model) == crate::data::target_count(&*data);
}
fn copy(target: &mut [u8], start: i32, source: &[u8]) -> () {
    for index in 0..source.len() { target[start + index] = source[index]; }
    return;
}
fn combine(training: &[u8], data: &[u8], sampler: &[u8]) -> TensorResult<ByteBuf>{
    if training.len() > 16777180 || data.len() > 16777180 - training.len() { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }
    let first = training.len() + data.len();
    if sampler.len() > 16777180 - first { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }
    let end = 32 + first + sampler.len();
    let made = crate::data::allocate(end + 4);
    match made {
        TensorResult::Ok(bytes) => {
            let mut output = bytes;
            let magic = b"OXDP0001";
            copy(&mut output, 0, &magic);
            let training_length = training.len();
            let data_length = data.len();
            let sampler_length = sampler.len();
            crate::codec::put64(&mut output, 8, training_length.to_u64_checked());
            crate::codec::put64(&mut output, 16, data_length.to_u64_checked());
            crate::codec::put64(&mut output, 24, sampler_length.to_u64_checked());
            copy(&mut output, 32, &*training);
            copy(&mut output, 32 + training.len(), &*data);
            copy(&mut output, 32 + first, &*sampler);
            let crc = crate::codec::checksum(&output, end);
            crate::codec::put32(&mut output, end, crc);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
    }
}
pub fn encode(model: &Model, state: &State, data: &Dataset, sampler: &Sampler) -> TensorResult<ByteBuf>{
    if !matching(&*model, &*data, &*sampler) { return TensorResult::<ByteBuf>:: Err(Error::ShapeMismatch); }
    let training = crate::model::session::encode(&*model, &*state);
    match training {
        TensorResult::Ok(first) => {
            let dataset = crate::data::checkpoint::encode(&*data);
            match dataset {
                TensorResult::Ok(second) => {
                    let sampling = crate::data::sampler_checkpoint::encode(&*sampler);
                    match sampling { TensorResult::Ok(third) => { return combine(&first, &second, &third); }, TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); } }
                },
                TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
    }
}

fn header_valid(bytes: &[u8]) -> bool {
    if bytes.len() < 36 || bytes.len() > 16777216 { return false; }
    let magic = b"OXDP0001";
    for index in 0..8 { if bytes[index] != magic[index] { return false; } }
    let first = crate::codec::get64(&*bytes, 8);
    let second = crate::codec::get64(&*bytes, 16);
    let third = crate::codec::get64(&*bytes, 24);
    let available_length = bytes.len() - 36;
    let available = available_length.to_u64_checked();
    if first > available || second > available - first || third != available - first - second { return false; }
    let end = bytes.len() - 4;
    return crate::codec::get32(&*bytes, end) == crate::codec::checksum(&*bytes, end);
}

pub fn decode(bytes: &[u8]) -> TensorResult<Restored>{
    if !header_valid(&*bytes) { return TensorResult::<Restored>:: Err(Error::InvalidCheckpoint); }
    let training_length = crate::codec::get64(&*bytes, 8);
    let data_length = crate::codec::get64(&*bytes, 16);
    let first = 32 + training_length.to_i32_checked();
    let second = first + data_length.to_i32_checked();
    let training = crate::model::session::decode(&*bytes[32..first]);
    match training {
        TensorResult::Ok(ready) => {
            let Decoded { model: model, optimizer: state, legacy: legacy } = ready;
            if legacy { return TensorResult::<Restored>:: Err(Error::InvalidCheckpoint); }
            let dataset = crate::data::checkpoint::decode(&*bytes[first..second]);
            match dataset {
                TensorResult::Ok(data) => {
                    let sampling = crate::data::sampler_checkpoint::decode(&*bytes[second..bytes.len() - 4]);
                    match sampling {
                        TensorResult::Ok(sampler) => {
                            if !matching(&model, &data, &sampler) { return TensorResult::<Restored>:: Err(Error::InvalidCheckpoint); }
                            return TensorResult::Ok(Restored { model: model, optimizer: state, dataset: data, sampler: sampler });
                        },
                        TensorResult::Err(error) => { return TensorResult::<Restored>:: Err(error); }
                    }
                },
                TensorResult::Err(error) => { return TensorResult::<Restored>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Restored>:: Err(error); }
    }
}
