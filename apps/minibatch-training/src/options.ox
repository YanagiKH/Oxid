use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use alloc::bytes::truncate;
use std::env::arg_bytes;
use std::env::arg_count;
use std::env::ArgStatus;
use crate::tensor::TensorResult;
use crate::tensor::Error;

pub struct Options { pub mode: i32, pub steps: i32, pub size: i32, pub seed: u32, pub algorithm: i32, pub source: ByteBuf, pub first: ByteBuf, pub second: ByteBuf }
fn grown(status: GrowStatus) -> bool {
    match status { GrowStatus::Ok => { return true; }, GrowStatus::CapacityOverflow => { return false; }, GrowStatus::AllocationFailed => { return false; }, GrowStatus::MemoryLimit => { return false; } }
}
fn argument(index: i32) -> TensorResult<ByteBuf>{
    let mut bytes = new();
    if !grown(try_resize(&mut bytes, 4095, 0u8)) { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); }
    let copied = arg_bytes(index, &mut bytes);
    match copied {
        ArgStatus::Copied(length) => { truncate(&mut bytes, length); return TensorResult::Ok(bytes); },
        ArgStatus::TooSmall(required) => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); },
        ArgStatus::Missing => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); },
        ArgStatus::TooLong => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }
    }
}
fn same(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() { return false; }
    for index in 0..left.len() { if left[index] != right[index] { return false; } } return true;
}
pub fn help_requested() -> bool {
    if arg_count() != 2 { return false; }
    let copied = argument(1);
    match copied { TensorResult::Ok(bytes) => { let long = b"--help"; let short = b"-h"; return same(&bytes, &long) || same(&bytes, &short); }, TensorResult::Err(error) => { return false; } }
}

fn number(bytes: &[u8], maximum: i32) -> i32 {
    if bytes.len() == 0 { return -1; }
    let mut count = 0;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if byte < 48u8 || byte > 57u8 { return -1; }
        let digit = byte.to_i32_checked() - 48;
        if count > (maximum - digit) / 10 { return -1; }
        count = count * 10 + digit;
    }
    if count == 0 { return -1; }
    return count;
}
fn numeric(index: i32, maximum: i32) -> i32 {
    let read = argument(index);
    match read { TensorResult::Ok(bytes) => { return number(&bytes, maximum); }, TensorResult::Err(error) => { return -1; } }
}
fn seed_value(bytes: &[u8]) -> TensorResult<u32>{
    if bytes.len() == 0 { return TensorResult::<u32>:: Err(Error::Bounds); }
    let mut value = 0u32;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if byte < 48u8 || byte > 57u8 { return TensorResult::<u32>:: Err(Error::Bounds); }
        let digit = byte.to_u32_checked() - 48u32;
        if value > (4294967295u32 - digit) / 10u32 { return TensorResult::<u32>:: Err(Error::Bounds); }
        value = value * 10u32 + digit;
    }
    return TensorResult::Ok(value);
}
fn selected_seed() -> TensorResult<u32>{
    if arg_count() < 8 { return TensorResult::Ok(42u32); }
    let read = argument(7);
    match read { TensorResult::Ok(bytes) => { return seed_value(&bytes); }, TensorResult::Err(error) => { return TensorResult::<u32>:: Err(error); } }
}
fn algorithm_argument() -> i32 {
    if arg_count() < 7 { return 0; }
    let read = argument(6);
    match read {
        TensorResult::Ok(bytes) => {
            let sgd = b"sgd";
            let adam = b"adam";
            let adamw = b"adamw";
            if same(&bytes, &sgd) { return 0; }
            if same(&bytes, &adam) { return 1; }
            if same(&bytes, &adamw) { return 2; }
            return -1;
        },
        TensorResult::Err(error) => { return -1; }
    }
}
fn paths(mode: i32, steps: i32, size: i32, seed: u32, algorithm: i32, source: ByteBuf, prefix: &[u8]) -> TensorResult<Options>{
    if prefix.len() == 0 || prefix.len() > 4093 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let mut first = new();
    let mut second = new();
    let a = b".a";
    let b = b".b";
    if !grown(try_extend(&mut first, &*prefix)) || !grown(try_extend(&mut first, &a)) || !grown(try_extend(&mut second, &*prefix)) || !grown(try_extend(&mut second, &b)) { return TensorResult::<Options>:: Err(Error::AllocationFailed); }
    return TensorResult::Ok(Options { mode: mode, steps: steps, size: size, seed: seed, algorithm: algorithm, source: source, first: first, second: second });
}
fn train() -> TensorResult<Options>{
    if arg_count() < 6 || arg_count() > 8 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let steps = numeric(4, 1000000);
    let size = numeric(5, 2147483647);
    let algorithm = algorithm_argument();
    if steps < 0 || size < 0 || algorithm < 0 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let seed_arg = selected_seed();
    match seed_arg {
        TensorResult::Ok(seed) => {
            let source_arg = argument(2);
            match source_arg {
                TensorResult::Ok(source) => {
                    let prefix_arg = argument(3);
                    match prefix_arg { TensorResult::Ok(prefix) => { return paths(1, steps, size, seed, algorithm, source, &prefix); }, TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); } }
                },
                TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); }
    }
}
fn resume() -> TensorResult<Options>{
    if arg_count() != 4 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let steps = numeric(3, 1000000);
    if steps < 0 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let prefix_arg = argument(2);
    match prefix_arg { TensorResult::Ok(prefix) => { return paths(2, steps, 1, 42u32, 0, new(), &prefix); }, TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); } }
}
pub fn parse() -> TensorResult<Options>{
    if arg_count() < 2 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let read = argument(1);
    match read {
        TensorResult::Ok(bytes) => {
            let start = b"train";
            let restore = b"resume";
            if same(&bytes, &start) { return train(); }
            if same(&bytes, &restore) { return resume(); }
            return TensorResult::<Options>:: Err(Error::Bounds);
        },
        TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); }
    }
}
