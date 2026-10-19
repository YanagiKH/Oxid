use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::len as bytes_len;
use alloc::bytes::try_extend;
use alloc::bytes::truncate as truncate_bytes;
use alloc::bytes::take as take_bytes;

pub struct I32Vec {
    bytes: ByteBuf
}

pub enum Element {
    Value(i32),
    Missing
}

pub enum FromBytes {
    Ok(I32Vec),
    Invalid(ByteBuf)
}

// Normalize a signed remainder before division. In particular MIN % 256 is 0.
fn encode_le(value: i32, output: &mut [u8]) -> () {
    let mut remaining = value;
    let mut offset = 0;
    while offset < 4 {
        let mut remainder = remaining % 256;
        if remainder < 0 {
            remainder = remainder + 256;
        }
        output[offset] = remainder.to_u8_checked();
        remaining = (remaining - remainder) / 256;
        offset = offset + 1;
    }
    return;
}

fn decode_le(input: &[u8], offset: i32) -> i32 {
    let high_byte = input[offset + 3];
    let mut accumulator = high_byte.to_i32();
    if accumulator >= 128 {
        accumulator = accumulator - 256;
    }
    let third_byte = input[offset + 2];
    accumulator = accumulator * 256 + third_byte.to_i32();
    let second_byte = input[offset + 1];
    accumulator = accumulator * 256 + second_byte.to_i32();
    let first_byte = input[offset];
    return accumulator * 256 + first_byte.to_i32();
}

fn write_le(output: &mut [u8], offset: i32, encoded: &[u8]) -> () {
    output[offset] = encoded[0];
    output[offset + 1] = encoded[1];
    output[offset + 2] = encoded[2];
    output[offset + 3] = encoded[3];
    return;
}

pub fn new_vec() -> I32Vec {
    return I32Vec { bytes: new_bytes() };
}

pub fn len(vector: &I32Vec) -> i32 {
    return bytes_len(&*vector.bytes) / 4;
}

pub fn get(vector: &I32Vec, index: i32) -> Element {
    if index < 0 || index >= len(&*vector) {
        return Element::Missing;
    }
    return Element::Value(decode_le(&*vector.bytes, index * 4));
}

pub fn set(vector: &mut I32Vec, index: i32, value: i32) -> bool {
    if index < 0 || index >= len(&*vector) {
        return false;
    }
    let zero = 0;
    let mut encoded = [zero.to_u8_checked(), zero.to_u8_checked(), zero.to_u8_checked(), zero.to_u8_checked()];
    encode_le(value, &mut encoded);
    write_le(&mut *vector.bytes, index * 4, &encoded);
    return true;
}

pub fn try_push(vector: &mut I32Vec, value: i32) -> GrowStatus {
    let zero = 0;
    let mut encoded = [zero.to_u8_checked(), zero.to_u8_checked(), zero.to_u8_checked(), zero.to_u8_checked()];
    encode_le(value, &mut encoded);
    return try_extend(&mut *vector.bytes, &encoded);
}

pub fn pop(vector: &mut I32Vec) -> Element {
    let initialized = bytes_len(&*vector.bytes);
    if initialized == 0 {
        return Element::Missing;
    }
    let value = decode_le(&*vector.bytes, initialized - 4);
    truncate_bytes(&mut *vector.bytes, initialized - 4);
    return Element::Value(value);
}

pub fn from_le_bytes(input: ByteBuf) -> FromBytes {
    if bytes_len(&input) % 4 != 0 {
        return FromBytes::Invalid(input);
    }
    return FromBytes::Ok(I32Vec { bytes: input });
}

pub fn into_le_bytes(vector: I32Vec) -> ByteBuf {
    let mut owned = vector;
    return take_bytes(&mut owned.bytes);
}
