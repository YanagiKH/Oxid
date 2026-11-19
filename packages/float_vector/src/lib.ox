use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::len as byte_len;
use alloc::bytes::capacity as byte_capacity;
use alloc::bytes::try_reserve as reserve_bytes;
use alloc::bytes::try_extend as extend_bytes;
use alloc::bytes::truncate as truncate_bytes;
use alloc::bytes::take as take_bytes;

pub struct Vector {
    bytes: ByteBuf
}

pub enum Read {
    Value(f64),
    OutOfBounds
}

pub enum FromBytes {
    Ok(Vector),
    InvalidLength(ByteBuf)
}

fn encode(value: f64) -> [u8; 8] {
    let mut bits = value.to_bits();
    let mut output = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let mut index = 0;
    while index < 8 {
        let low = bits & 255u64;
        output[index] = low.to_u8_checked();
        bits = bits >> 8;
        index = index + 1;
    }
    return output;
}

fn decode(input: &[u8], byte_offset: i32) -> f64 {
    let mut bits = 0u64;
    let mut index = 0;
    while index < 8 {
        let current = input[byte_offset + index];
        let wide = current.to_u64_checked();
        bits = bits | (wide << (index * 8));
        index = index + 1;
    }
    return f64::from_bits(bits);
}

fn write(output: &mut [u8], byte_offset: i32, value: f64) -> () {
    let encoded = encode(value);
    let mut index = 0;
    while index < 8 {
        output[byte_offset + index] = encoded[index];
        index = index + 1;
    }
    return;
}

pub fn new() -> Vector {
    return Vector { bytes: new_bytes() };
}

pub fn len(vector: &Vector) -> i32 {
    return byte_len(&*vector.bytes) / 8;
}

pub fn capacity(vector: &Vector) -> i32 {
    return byte_capacity(&*vector.bytes) / 8;
}

pub fn try_reserve(vector: &mut Vector, additional: i32) -> GrowStatus {
    let current_count = len(&*vector);
    if additional < 0 || additional > 268435455 - current_count {
        return GrowStatus::CapacityOverflow;
    }
    return reserve_bytes(&mut *vector.bytes, additional * 8);
}

pub fn try_push(vector: &mut Vector, value: f64) -> GrowStatus {
    let encoded = encode(value);
    return extend_bytes(&mut *vector.bytes, &encoded);
}

pub fn get(vector: &Vector, index: i32) -> Read {
    if index < 0 || index >= len(&*vector) {
        return Read::OutOfBounds;
    }
    return Read::Value(decode(&*vector.bytes, index * 8));
}

pub fn set(vector: &mut Vector, index: i32, value: f64) -> bool {
    if index < 0 || index >= len(&*vector) {
        return false;
    }
    write(&mut *vector.bytes, index * 8, value);
    return true;
}

pub fn truncate(vector: &mut Vector, item_count: i32) -> bool {
    if item_count < 0 || item_count > len(&*vector) {
        return false;
    }
    truncate_bytes(&mut *vector.bytes, item_count * 8);
    return true;
}

pub fn try_extend(vector: &mut Vector, source: &Vector) -> GrowStatus {
    return extend_bytes(&mut *vector.bytes, &*source.bytes);
}

pub fn take(vector: &mut Vector) -> Vector {
    return Vector { bytes: take_bytes(&mut *vector.bytes) };
}

pub fn into_bytes(input: Vector) -> ByteBuf {
    let mut vector = input;
    return take_bytes(&mut vector.bytes);
}

pub fn from_bytes(input: ByteBuf) -> FromBytes {
    if byte_len(&input) % 8 != 0 {
        return FromBytes::InvalidLength(input);
    }
    return FromBytes::Ok(Vector { bytes: input });
}
