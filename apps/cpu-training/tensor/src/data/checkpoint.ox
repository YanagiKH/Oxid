use alloc::bytes::ByteBuf;
use crate::TensorResult;
use crate::Error;
use crate::data::Dataset;

fn elements(rows: i32, features: i32, targets: i32) -> i32 {
    if rows <= 0 || features <= 0 || targets <= 0 { return -1; }
    if features > 2097147 / rows { return -1; }
    let first = rows * features;
    if targets > (2097147 - first) / rows { return -1; }
    return first + rows * targets;
}

fn copy(target: &mut [u8], start: i32, source: &[u8]) -> () {
    for index in 0..source.len() { target[start + index] = source[index]; }
    return;
}

pub fn encode(data: &Dataset) -> TensorResult<ByteBuf>{
    let rows = data.inputs.rows;
    let features = data.inputs.columns;
    let targets = data.targets.columns;
    let count = elements(rows, features, targets);
    if count < 0 { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }
    let made = crate::data::allocate(36 + count * 8);
    match made {
        TensorResult::Ok(bytes) => {
            let mut output = bytes;
            let magic = b"OXDS0001";
            copy(&mut output, 0, &magic);
            crate::codec::put32(&mut output, 8, rows.to_u32_checked());
            crate::codec::put32(&mut output, 12, features.to_u32_checked());
            crate::codec::put32(&mut output, 16, targets.to_u32_checked());
            let payload = count * 8;
            crate::codec::put64(&mut output, 24, payload.to_u64_checked());
            copy(&mut output, 32, &*data.inputs.data);
            copy(&mut output, 32 + rows * features * 8, &*data.targets.data);
            let end = 32 + count * 8;
            let crc = crate::codec::checksum(&output, end);
            crate::codec::put32(&mut output, end, crc);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
    }
}

fn valid(bytes: &[u8]) -> bool {
    if bytes.len() < 36 || bytes.len() > 16777216 { return false; }
    let magic = b"OXDS0001";
    for index in 0..8 { if bytes[index] != magic[index] { return false; } }
    if crate::codec::get32(&*bytes, 20) != 0u32 { return false; }
    let rows = crate::codec::get32(&*bytes, 8);
    let features = crate::codec::get32(&*bytes, 12);
    let targets = crate::codec::get32(&*bytes, 16);
    if rows > 2097147u32 || features > 2097147u32 || targets > 2097147u32 { return false; }
    let count = elements(rows.to_i32_checked(), features.to_i32_checked(), targets.to_i32_checked());
    if count < 0 || bytes.len() != 36 + count * 8 { return false; }
    let payload = count * 8;
    if crate::codec::get64(&*bytes, 24) != payload.to_u64_checked() { return false; }
    let end = bytes.len() - 4;
    if crate::codec::get32(&*bytes, end) != crate::codec::checksum(&*bytes, end) { return false; }
    for index in 0..count {
        let bits = crate::codec::get64(&*bytes, 32 + index * 8);
        if ((bits >> 52) & 2047u64) == 2047u64 { return false; }
    }
    return true;
}

pub fn decode(bytes: &[u8]) -> TensorResult<Dataset>{
    if !valid(&*bytes) { return TensorResult::<Dataset>:: Err(Error::InvalidCheckpoint); }
    let row_bits = crate::codec::get32(&*bytes, 8);
    let feature_bits = crate::codec::get32(&*bytes, 12);
    let target_bits = crate::codec::get32(&*bytes, 16);
    let rows = row_bits.to_i32_checked();
    let features = feature_bits.to_i32_checked();
    let targets = target_bits.to_i32_checked();
    let first = crate::filled(rows, features, 0.0);
    match first {
        TensorResult::Ok(inputs) => {
            let mut input = inputs;
            let other = crate::filled(rows, targets, 0.0);
            match other {
                TensorResult::Ok(target_values) => {
                    let mut target = target_values;
                    copy(&mut input.data, 0, &*bytes[32..32 + rows * features * 8]);
                    copy(&mut target.data, 0, &*bytes[32 + rows * features * 8..bytes.len() - 4]);
                    return TensorResult::Ok(Dataset { inputs: input, targets: target });
                },
                TensorResult::Err(error) => { return TensorResult::<Dataset>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Dataset>:: Err(error); }
    }
}
