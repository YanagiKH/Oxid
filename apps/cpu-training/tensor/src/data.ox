use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::len;
use crate::Tensor;
use crate::TensorResult;
use crate::Error;
pub mod checkpoint;
pub mod sampler_checkpoint;
pub mod session;

pub struct Dataset { inputs: Tensor, targets: Tensor }
pub struct Sampler { rows: i32, size: i32, shuffle: bool, rng: u32, cursor: i32, epoch: u64, batches: u64, order: ByteBuf }
pub struct Batch { pub inputs: Tensor, pub targets: Tensor }

fn finite(number: f64) -> bool { return ((number.to_bits() >> 52) & 2047u64) != 2047u64; }
fn finite_tensor(value: &Tensor) -> bool {
    for index in 0..value.rows * value.columns { if !finite(crate::codec::load(&*value.data, index)) { return false; } }
    return true;
}

pub fn create(inputs: Tensor, targets: Tensor) -> TensorResult<Dataset>{
    if !crate::valid(&inputs) || !crate::valid(&targets) { return TensorResult::<Dataset>:: Err(Error::InvalidStorage); }
    if inputs.rows <= 0 || inputs.columns <= 0 || targets.rows <= 0 || targets.columns <= 0 { return TensorResult::<Dataset>:: Err(Error::NegativeShape); }
    if inputs.rows != targets.rows { return TensorResult::<Dataset>:: Err(Error::ShapeMismatch); }
    if !finite_tensor(&inputs) || !finite_tensor(&targets) { return TensorResult::<Dataset>:: Err(Error::NonFinite); }
    return TensorResult::Ok(Dataset { inputs: inputs, targets: targets });
}

pub fn row_count(data: &Dataset) -> i32 { return data.inputs.rows; }
pub fn feature_count(data: &Dataset) -> i32 { return data.inputs.columns; }
pub fn target_count(data: &Dataset) -> i32 { return data.targets.columns; }
pub fn input_at(data: &Dataset, row: i32, column: i32) -> TensorResult<f64>{ return crate::get(&*data.inputs, row, column); }
pub fn target_at(data: &Dataset, row: i32, column: i32) -> TensorResult<f64>{ return crate::get(&*data.targets, row, column); }
pub fn loss(model: &crate::model::Model, data: &Dataset) -> TensorResult<f64>{ return crate::model::loss(&*model, &*data.inputs, &*data.targets); }
pub fn epoch(sampler: &Sampler) -> u64 { return sampler.epoch; }
pub fn position(sampler: &Sampler) -> i32 { return sampler.cursor; }
pub fn batch_count(sampler: &Sampler) -> u64 { return sampler.batches; }
pub fn batch_size(sampler: &Sampler) -> i32 { return sampler.size; }
pub fn shuffled(sampler: &Sampler) -> bool { return sampler.shuffle; }
pub fn random_state(sampler: &Sampler) -> u32 { return sampler.rng; }

fn random(sampler: &mut Sampler) -> u32 {
    let mut bits = sampler.rng;
    bits = bits ^ (bits << 13);
    bits = bits ^ (bits >> 17);
    bits = bits ^ (bits << 5);
    sampler.rng = bits;
    return bits;
}

fn uniform(sampler: &mut Sampler, bound: u32) -> u32 {
    let limit = 4294967295u32 - 4294967295u32 % bound;
    while true {
        let draw = random(&mut *sampler) - 1u32;
        if draw < limit { return draw % bound; }
    }
    return 0u32;
}

fn reorder(sampler: &mut Sampler) -> () {
    for row in 0..sampler.rows { crate::codec::put32(&mut *sampler.order, row * 4, row.to_u32_checked()); }
    if sampler.shuffle {
        let mut index = sampler.rows - 1;
        while index > 0 {
            let width = index + 1;
            let bound = width.to_u32_checked();
            let draw = uniform(&mut *sampler, bound);
            let other = draw.to_i32_checked();
            let first = crate::codec::get32(&*sampler.order, index * 4);
            let second = crate::codec::get32(&*sampler.order, other * 4);
            crate::codec::put32(&mut *sampler.order, index * 4, second);
            crate::codec::put32(&mut *sampler.order, other * 4, first);
            index = index - 1;
        }
    }
    return;
}

fn allocate(length: i32) -> TensorResult<ByteBuf>{
    let mut bytes = new();
    let grown = try_resize(&mut bytes, length, 0u8);
    match grown { GrowStatus::Ok => { return TensorResult::Ok(bytes); }, GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }, GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); }, GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); } }
}

pub fn initialize_sampler(data: &Dataset, size: i32, shuffle: bool, seed: u32) -> TensorResult<Sampler>{
    if size <= 0 { return TensorResult::<Sampler>:: Err(Error::NegativeShape); }
    let rows = row_count(&*data);
    if rows > 536870911 { return TensorResult::<Sampler>:: Err(Error::SizeOverflow); }
    let made = allocate(rows * 4);
    match made {
        TensorResult::Ok(order) => {
            let mut rng = seed;
            if rng == 0u32 { rng = 1831565813u32; }
            let mut output = Sampler { rows: rows, size: size, shuffle: shuffle, rng: rng, cursor: 0, epoch: 0u64, batches: 0u64, order: order };
            reorder(&mut output);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<Sampler>:: Err(error); }
    }
}

fn per_epoch(rows: i32, size: i32) -> i32 { return (rows - 1) / size + 1; }
fn valid(sampler: &Sampler) -> bool {
    if sampler.rows <= 0 || sampler.rows > 536870911 || sampler.size <= 0 || sampler.rng == 0u32 { return false; }
    if len(&*sampler.order) != sampler.rows * 4 { return false; }
    let total = per_epoch(sampler.rows, sampler.size);
    let count = total.to_u64_checked();
    if sampler.epoch != sampler.batches / count { return false; }
    let offset = sampler.batches % count;
    let batch = offset.to_i32_checked();
    return sampler.cursor == batch * sampler.size;
}

pub fn next_size(sampler: &Sampler) -> i32 {
    if !valid(&*sampler) { return -1; }
    let remaining = sampler.rows - sampler.cursor;
    if remaining < sampler.size { return remaining; }
    return sampler.size;
}

pub fn row_index(sampler: &Sampler, offset: i32) -> TensorResult<i32>{
    let count = next_size(&*sampler);
    if count < 0 { return TensorResult::<i32>:: Err(Error::InvalidStorage); }
    if offset < 0 || offset >= count { return TensorResult::<i32>:: Err(Error::Bounds); }
    let index = crate::codec::get32(&*sampler.order, (sampler.cursor + offset) * 4);
    return TensorResult::Ok(index.to_i32_checked());
}

fn gather(source: &Tensor, sampler: &Sampler, target: &mut Tensor) -> () {
    let columns = target.columns;
    for row in 0..target.rows {
        let bits = crate::codec::get32(&*sampler.order, (sampler.cursor + row) * 4);
        let index = bits.to_i32_checked();
        for column in 0..columns {
            let value = crate::codec::load(&*source.data, index * source.columns + column);
            crate::codec::store(&mut *target.data, row * columns + column, value);
        }
    }
    return;
}

pub fn prepare(data: &Dataset, sampler: &Sampler) -> TensorResult<Batch>{
    let count = next_size(&*sampler);
    if count < 0 { return TensorResult::<Batch>:: Err(Error::InvalidStorage); }
    if sampler.rows != row_count(&*data) { return TensorResult::<Batch>:: Err(Error::ShapeMismatch); }
    let made = crate::filled(count, feature_count(&*data), 0.0);
    match made {
        TensorResult::Ok(inputs) => {
            let mut first = inputs;
            let other = crate::filled(count, target_count(&*data), 0.0);
            match other {
                TensorResult::Ok(targets) => {
                    let mut second = targets;
                    gather(&*data.inputs, &*sampler, &mut first);
                    gather(&*data.targets, &*sampler, &mut second);
                    return TensorResult::Ok(Batch { inputs: first, targets: second });
                },
                TensorResult::Err(error) => { return TensorResult::<Batch>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Batch>:: Err(error); }
    }
}

fn advance(sampler: &mut Sampler) -> () {
    let total = per_epoch(sampler.rows, sampler.size);
    let count = total.to_u64_checked();
    sampler.batches = sampler.batches + 1u64;
    sampler.epoch = sampler.batches / count;
    let offset = sampler.batches % count;
    sampler.cursor = offset.to_i32_checked() * sampler.size;
    if sampler.cursor == 0 { reorder(&mut *sampler); }
    return;
}

pub fn next_batch(data: &Dataset, sampler: &mut Sampler) -> TensorResult<Batch>{
    if sampler.batches == 18446744073709551615u64 { return TensorResult::<Batch>:: Err(Error::StepOverflow); }
    let made = prepare(&*data, &*sampler);
    match made {
        TensorResult::Ok(batch) => { advance(&mut *sampler); return TensorResult::Ok(batch); },
        TensorResult::Err(error) => { return TensorResult::<Batch>:: Err(error); }
    }
}

pub fn train_next(model: &mut crate::model::Model, state: &mut crate::optimizer::State, data: &Dataset, sampler: &mut Sampler) -> TensorResult<f64>{
    if sampler.batches == 18446744073709551615u64 { return TensorResult::<f64>:: Err(Error::StepOverflow); }
    let made = prepare(&*data, &*sampler);
    match made {
        TensorResult::Ok(batch) => {
            let trained = crate::model::train_with_optimizer(&mut *model, &mut *state, &batch.inputs, &batch.targets);
            match trained { TensorResult::Ok(objective) => { advance(&mut *sampler); return TensorResult::Ok(objective); }, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); } }
        },
        TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }
    }
}
