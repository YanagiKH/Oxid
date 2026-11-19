use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::len as byte_len;
use alloc::bytes::try_extend as extend_bytes;
use crate::vector::Vector;
use crate::vector::Read;
use crate::vector::FromBytes;
use crate::vector::new;
use crate::vector::len;
use crate::vector::capacity;
use crate::vector::try_reserve;
use crate::vector::try_push;
use crate::vector::try_extend;
use crate::vector::get;
use crate::vector::set;
use crate::vector::truncate;
use crate::vector::take;
use crate::vector::into_bytes;
use crate::vector::from_bytes;

fn grown(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn has_bits(vector: &Vector, index: i32, expected: u64) -> bool {
    let found = get(&*vector, index);
    match found {
        Read::Value(value) => { return value.to_bits() == expected; },
        Read::OutOfBounds => { return false; },
    }
}

fn missing(vector: &Vector, index: i32) -> bool {
    let found = get(&*vector, index);
    match found {
        Read::Value(value) => { return false; },
        Read::OutOfBounds => { return true; },
    }
}

fn overflow(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return false; },
        GrowStatus::CapacityOverflow => { return true; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn memory_limited(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return false; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return true; },
    }
}

fn same_bytes(input: &[u8], expected: &[u8]) -> bool {
    if input.len() != expected.len() { return false; }
    let mut index = 0;
    while index < input.len() {
        if input[index] != expected[index] { return false; }
        index = index + 1;
    }
    return true;
}

fn check_values() -> bool {
    let mut vector = new();
    let patterns = [0u64, 9223372036854775808u64, 1u64, 9223372036854775809u64,
        4503599627370495u64, 4503599627370496u64, 9218868437227405311u64,
        9218868437227405312u64, 18442240474082181120u64,
        9218868437227405313u64, 9221120237041090561u64,
        18444492273895866369u64, 18446744073709551615u64];
    let mut index = 0;
    while index < patterns.len() {
        let bits = patterns[index];
        let value = f64::from_bits(bits);
        if !grown(try_push(&mut vector, value)) { return false; }
        index = index + 1;
    }
    if len(&vector) != patterns.len() { return false; }
    index = 0;
    while index < patterns.len() {
        if !has_bits(&vector, index, patterns[index]) { return false; }
        index = index + 1;
    }
    let mut state = 88172645463325252u64;
    index = 0;
    while index < 128 {
        state = state ^ (state << 13);
        state = state ^ (state >> 7);
        state = state ^ (state << 17);
        let value = f64::from_bits(state);
        if !grown(try_push(&mut vector, value)) { return false; }
        if !has_bits(&vector, patterns.len() + index, state) { return false; }
        if !set(&mut vector, index, value) || !has_bits(&vector, index, state) { return false; }
        index = index + 1;
    }
    if !missing(&vector, -1) || !missing(&vector, len(&vector)) || !missing(&vector, 2147483647) { return false; }
    let end_index = len(&vector);
    if set(&mut vector, -1, 0.0) || set(&mut vector, end_index, 0.0) { return false; }
    let previous_capacity = capacity(&vector);
    if !overflow(try_reserve(&mut vector, -1)) || !overflow(try_reserve(&mut vector, 268435455)) || !overflow(try_reserve(&mut vector, 2147483647)) { return false; }
    if !memory_limited(try_reserve(&mut vector, 3000000)) { return false; }
    if capacity(&vector) != previous_capacity || len(&vector) != patterns.len() + 128 { return false; }
    if !has_bits(&vector, patterns.len() + 127, state) { return false; }
    if truncate(&mut vector, -1) || truncate(&mut vector, 2147483647) { return false; }
    if !truncate(&mut vector, 1) || len(&vector) != 1 || !missing(&vector, 1) { return false; }
    if !grown(try_push(&mut vector, 9.25)) || !has_bits(&vector, 1, 4621396905123905536u64) { return false; }
    return true;
}

fn check_transfer() -> bool {
    let mut empty = new();
    if len(&empty) != 0 || capacity(&empty) != 0 || !missing(&empty, 0) { return false; }
    if !grown(try_reserve(&mut empty, 0)) || capacity(&empty) != 0 { return false; }
    let mut source = new();
    if !grown(try_reserve(&mut source, 4)) || len(&source) != 0 || capacity(&source) < 4 { return false; }
    if !grown(try_push(&mut source, 1.5)) || !grown(try_push(&mut source, -0.0)) { return false; }
    if !grown(try_extend(&mut empty, &source)) || len(&empty) != 2 { return false; }
    if !has_bits(&empty, 0, 4609434218613702656u64) || !has_bits(&empty, 1, 9223372036854775808u64) { return false; }
    let old_capacity = capacity(&source);
    let moved = take(&mut source);
    if len(&source) != 0 || capacity(&source) != 0 || capacity(&moved) != old_capacity { return false; }
    if !has_bits(&moved, 0, 4609434218613702656u64) { return false; }
    if !grown(try_push(&mut source, 0.0)) || len(&source) != 1 { return false; }
    let bytes = into_bytes(moved);
    if byte_len(&bytes) != 16 { return false; }
    let decoded = from_bytes(bytes);
    match decoded {
        FromBytes::Ok(restored) => {
            if len(&restored) != 2 || !has_bits(&restored, 1, 9223372036854775808u64) { return false; }
        },
        FromBytes::InvalidLength(rejected) => { return false; },
    }
    return true;
}

fn check_bytes() -> bool {
    let encoded = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 240u8, 63u8,
        1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 248u8, 127u8];
    let mut input = new_bytes();
    if !grown(extend_bytes(&mut input, &encoded)) { return false; }
    let decoded = from_bytes(input);
    match decoded {
        FromBytes::Ok(vector) => {
            if !has_bits(&vector, 0, 4607182418800017408u64) || !has_bits(&vector, 1, 9221120237041090561u64) { return false; }
            let bytes = into_bytes(vector);
            if !same_bytes(&bytes, &encoded) { return false; }
        },
        FromBytes::InvalidLength(rejected) => { return false; },
    }
    let invalid = [7u8, 6u8, 5u8, 4u8, 3u8, 2u8, 1u8];
    let mut malformed = new_bytes();
    if !grown(extend_bytes(&mut malformed, &invalid)) { return false; }
    let refused = from_bytes(malformed);
    match refused {
        FromBytes::Ok(vector) => { return false; },
        FromBytes::InvalidLength(rejected) => {
            if byte_len(&rejected) != invalid.len() { return false; }
            if !same_bytes(&rejected, &invalid) { return false; }
        },
    }
    return true;
}

fn main() -> i32 {
    if !check_values() { return 1; }
    if !check_transfer() { return 2; }
    if !check_bytes() { return 3; }
    return 0;
}
