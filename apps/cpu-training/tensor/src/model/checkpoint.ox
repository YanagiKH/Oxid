use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use crate::Tensor;
use crate::TensorResult;
use crate::Error;
use crate::model::Model;

fn put32(bytes: &mut [u8], start: i32, number: u32) -> () {
    for offset in 0..4 { let part = (number >> (offset * 8)) & 255u32; bytes[start + offset] = part.to_u8_checked(); }
    return;
}

fn get32(bytes: &[u8], start: i32) -> u32 {
    let mut number = 0u32;
    for offset in 0..4 { let part = bytes[start + offset]; number = number | (part.to_u32_checked() << (offset * 8)); }
    return number;
}

fn put64(bytes: &mut [u8], start: i32, number: u64) -> () {
    for offset in 0..8 { let part = (number >> (offset * 8)) & 255u64; bytes[start + offset] = part.to_u8_checked(); }
    return;
}

fn get64(bytes: &[u8], start: i32) -> u64 {
    let mut number = 0u64;
    for offset in 0..8 { let part = bytes[start + offset]; number = number | (part.to_u64_checked() << (offset * 8)); }
    return number;
}

fn checksum(bytes: &[u8], length: i32) -> u32 {
    let mut crc = 4294967295u32;
    for index in 0..length {
        let part = bytes[index];
        crc = crc ^ part.to_u32_checked();
        for bit in 0..8 {
            if (crc & 1u32) == 1u32 { crc = (crc >> 1) ^ 3988292384u32; }
            else { crc = crc >> 1; }
        }
    }
    return crc ^ 4294967295u32;
}

fn parameter_count(inputs: i32, hidden: i32, outputs: i32) -> i32 {
    if inputs <= 0 || hidden <= 0 || outputs <= 0 { return -1; }
    if hidden > 268435451 / inputs { return -1; }
    let first = inputs * hidden;
    if hidden > 268435451 - first { return -1; }
    let second = first + hidden;
    if outputs > (268435451 - second) / hidden { return -1; }
    let third = second + hidden * outputs;
    if outputs > 268435451 - third { return -1; }
    return third + outputs;
}

fn copy_into(target: &mut [u8], start: i32, source: &[u8]) -> () {
    for index in 0..source.len() { target[start + index] = source[index]; }
    return;
}

fn copy_out(source: &[u8], start: i32, target: &mut [u8]) -> () {
    for index in 0..target.len() { target[index] = source[start + index]; }
    return;
}

fn model_valid(model: &Model) -> bool {
    if !crate::valid(&*model.weights1) || !crate::valid(&*model.bias1) || !crate::valid(&*model.weights2) || !crate::valid(&*model.bias2) { return false; }
    let inputs = model.weights1.rows;
    let hidden = model.weights1.columns;
    let outputs = model.weights2.columns;
    if parameter_count(inputs, hidden, outputs) < 0 || model.rng == 0u32 { return false; }
    return model.bias1.rows == 1 && model.bias1.columns == hidden && model.weights2.rows == hidden && model.bias2.rows == 1 && model.bias2.columns == outputs;
}

pub fn encode(model: &Model) -> TensorResult<ByteBuf>{
    if !model_valid(&*model) { return TensorResult::<ByteBuf>:: Err(Error::InvalidStorage); }
    let inputs = model.weights1.rows;
    let hidden = model.weights1.columns;
    let outputs = model.weights2.columns;
    let elements = parameter_count(inputs, hidden, outputs);
    let length = 36 + elements * 8;
    let mut bytes = new();
    let grown = try_resize(&mut bytes, length, 0u8);
    match grown {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); },
        GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); }
    }
    let magic = [79u8, 88u8, 77u8, 68u8, 48u8, 48u8, 48u8, 49u8];
    copy_into(&mut bytes, 0, &magic);
    put32(&mut bytes, 8, inputs.to_u32_checked());
    put32(&mut bytes, 12, hidden.to_u32_checked());
    put32(&mut bytes, 16, outputs.to_u32_checked());
    put32(&mut bytes, 20, model.rng);
    put64(&mut bytes, 24, model.updates);
    let first = 32;
    let second = first + inputs * hidden * 8;
    let third = second + hidden * 8;
    let fourth = third + hidden * outputs * 8;
    copy_into(&mut bytes, first, &*model.weights1.data);
    copy_into(&mut bytes, second, &*model.bias1.data);
    copy_into(&mut bytes, third, &*model.weights2.data);
    copy_into(&mut bytes, fourth, &*model.bias2.data);
    let crc = checksum(&bytes, length - 4);
    put32(&mut bytes, length - 4, crc);
    return TensorResult::Ok(bytes);
}

fn finite_payload(bytes: &[u8], elements: i32) -> bool {
    for index in 0..elements {
        let bits = get64(&*bytes, 32 + index * 8);
        if ((bits >> 52) & 2047u64) == 2047u64 { return false; }
    }
    return true;
}

pub fn decode(bytes: &[u8]) -> TensorResult<Model>{
    if bytes.len() < 36 { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); }
    let magic = [79u8, 88u8, 77u8, 68u8, 48u8, 48u8, 48u8, 49u8];
    for index in 0..8 { if bytes[index] != magic[index] { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); } }
    if checksum(&*bytes, bytes.len() - 4) != get32(&*bytes, bytes.len() - 4) { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); }
    let input_bits = get32(&*bytes, 8);
    let hidden_bits = get32(&*bytes, 12);
    let output_bits = get32(&*bytes, 16);
    if input_bits > 268435451u32 || hidden_bits > 268435451u32 || output_bits > 268435451u32 { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); }
    let inputs = input_bits.to_i32_checked();
    let hidden = hidden_bits.to_i32_checked();
    let outputs = output_bits.to_i32_checked();
    let elements = parameter_count(inputs, hidden, outputs);
    if elements < 0 { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); }
    if bytes.len() != 36 + elements * 8 || get32(&*bytes, 20) == 0u32 { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); }
    if !finite_payload(&*bytes, elements) { return TensorResult::<Model>:: Err(Error::InvalidCheckpoint); }
    return materialize(&*bytes, inputs, hidden, outputs);
}

fn materialize(bytes: &[u8], inputs: i32, hidden: i32, outputs: i32) -> TensorResult<Model>{
    let mut weights1 = crate::empty();
    let mut bias1 = crate::empty();
    let mut weights2 = crate::empty();
    let mut bias2 = crate::empty();
    let first = crate::filled(inputs, hidden, 0.0);
    match first { TensorResult::Ok(tensor) => { weights1 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let second = crate::filled(1, hidden, 0.0);
    match second { TensorResult::Ok(tensor) => { bias1 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let third = crate::filled(hidden, outputs, 0.0);
    match third { TensorResult::Ok(tensor) => { weights2 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let fourth = crate::filled(1, outputs, 0.0);
    match fourth { TensorResult::Ok(tensor) => { bias2 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let offset1 = 32;
    let offset2 = offset1 + inputs * hidden * 8;
    let offset3 = offset2 + hidden * 8;
    let offset4 = offset3 + hidden * outputs * 8;
    copy_out(&*bytes, offset1, &mut weights1.data);
    copy_out(&*bytes, offset2, &mut bias1.data);
    copy_out(&*bytes, offset3, &mut weights2.data);
    copy_out(&*bytes, offset4, &mut bias2.data);
    return TensorResult::Ok(Model { weights1: weights1, bias1: bias1, weights2: weights2, bias2: bias2, rng: get32(&*bytes, 20), updates: get64(&*bytes, 24) });
}
