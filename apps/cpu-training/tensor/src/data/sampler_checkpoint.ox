use alloc::bytes::ByteBuf;
use crate::TensorResult;
use crate::Error;
use crate::data::Sampler;

fn copy(target: &mut [u8], start: i32, source: &[u8]) -> () {
    for index in 0..source.len() { target[start + index] = source[index]; }
    return;
}

pub fn encode(sampler: &Sampler) -> TensorResult<ByteBuf>{
    if !crate::data::valid(&*sampler) { return TensorResult::<ByteBuf>:: Err(Error::InvalidStorage); }
    if sampler.rows > 4194289 { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }
    let made = crate::data::allocate(60 + sampler.rows * 4);
    match made {
        TensorResult::Ok(bytes) => {
            let mut output = bytes;
            let magic = b"OXSP0001";
            copy(&mut output, 0, &magic);
            let rows = sampler.rows;
            let size = sampler.size;
            let cursor = sampler.cursor;
            crate::codec::put32(&mut output, 8, rows.to_u32_checked());
            crate::codec::put32(&mut output, 12, size.to_u32_checked());
            if sampler.shuffle { crate::codec::put32(&mut output, 16, 1u32); }
            crate::codec::put32(&mut output, 20, sampler.rng);
            crate::codec::put32(&mut output, 24, cursor.to_u32_checked());
            crate::codec::put64(&mut output, 32, sampler.epoch);
            crate::codec::put64(&mut output, 40, sampler.batches);
            let payload = sampler.rows * 4;
            crate::codec::put64(&mut output, 48, payload.to_u64_checked());
            copy(&mut output, 56, &*sampler.order);
            let end = 56 + sampler.rows * 4;
            let crc = crate::codec::checksum(&output, end);
            crate::codec::put32(&mut output, end, crc);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<ByteBuf>:: Err(error); }
    }
}

fn header_valid(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || bytes.len() > 16777216 { return false; }
    let magic = b"OXSP0001";
    for index in 0..8 { if bytes[index] != magic[index] { return false; } }
    let rows = crate::codec::get32(&*bytes, 8);
    let size = crate::codec::get32(&*bytes, 12);
    if rows == 0u32 || rows > 4194289u32 || size == 0u32 || size > 2147483647u32 { return false; }
    let length = rows.to_i32_checked() * 4;
    if bytes.len() != 60 + length || crate::codec::get64(&*bytes, 48) != length.to_u64_checked() { return false; }
    if crate::codec::get32(&*bytes, 16) > 1u32 || crate::codec::get32(&*bytes, 20) == 0u32 || crate::codec::get32(&*bytes, 28) != 0u32 { return false; }
    let total = crate::data::per_epoch(rows.to_i32_checked(), size.to_i32_checked());
    let count = total.to_u64_checked();
    let batches = crate::codec::get64(&*bytes, 40);
    if crate::codec::get64(&*bytes, 32) != batches / count { return false; }
    let cursor = crate::codec::get32(&*bytes, 24);
    if cursor.to_u64_checked() != (batches % count) * size.to_u64_checked() { return false; }
    let end = bytes.len() - 4;
    return crate::codec::get32(&*bytes, end) == crate::codec::checksum(&*bytes, end);
}

fn permutation(bytes: &[u8], seen: &mut [u8], rows: i32, shuffle: bool) -> bool {
    for row in 0..rows {
        let value = crate::codec::get32(&*bytes, 56 + row * 4);
        if value >= rows.to_u32_checked() { return false; }
        if !shuffle && value != row.to_u32_checked() { return false; }
        let index = value.to_i32_checked();
        let offset = index / 8;
        let mask = 1u8 << (index % 8);
        if (seen[offset] & mask) != 0u8 { return false; }
        seen[offset] = seen[offset] | mask;
    }
    return true;
}

fn materialize(bytes: &[u8], rows: i32, shuffle: bool) -> TensorResult<Sampler>{
    let made = crate::data::allocate(rows * 4);
    match made {
        TensorResult::Ok(order) => {
            let mut output = order;
            copy(&mut output, 0, &*bytes[56..56 + rows * 4]);
            let size = crate::codec::get32(&*bytes, 12);
            let cursor = crate::codec::get32(&*bytes, 24);
            return TensorResult::Ok(Sampler { rows: rows, size: size.to_i32_checked(), shuffle: shuffle, rng: crate::codec::get32(&*bytes, 20), cursor: cursor.to_i32_checked(), epoch: crate::codec::get64(&*bytes, 32), batches: crate::codec::get64(&*bytes, 40), order: output });
        },
        TensorResult::Err(error) => { return TensorResult::<Sampler>:: Err(error); }
    }
}

fn checked(bytes: &[u8], rows: i32, shuffle: bool) -> TensorResult<()>{
    let made = crate::data::allocate((rows - 1) / 8 + 1);
    match made {
        TensorResult::Ok(seen) => {
            let mut flags = seen;
            if !permutation(&*bytes, &mut flags, rows, shuffle) { return TensorResult::<()>:: Err(Error::InvalidCheckpoint); }
            return TensorResult::Ok(());
        },
        TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); }
    }
}

pub fn decode(bytes: &[u8]) -> TensorResult<Sampler>{
    if !header_valid(&*bytes) { return TensorResult::<Sampler>:: Err(Error::InvalidCheckpoint); }
    let row_bits = crate::codec::get32(&*bytes, 8);
    let rows = row_bits.to_i32_checked();
    let shuffle = crate::codec::get32(&*bytes, 16) == 1u32;
    let validated = checked(&*bytes, rows, shuffle);
    match validated { TensorResult::Ok(done) => { return materialize(&*bytes, rows, shuffle); }, TensorResult::Err(error) => { return TensorResult::<Sampler>:: Err(error); } }
}
