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

pub struct Options { pub mode: i32, pub steps: i32, pub algorithm: i32, pub first: ByteBuf, pub second: ByteBuf }
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
fn parse_steps(bytes: &[u8]) -> i32 {
    if bytes.len() == 0 { return -1; }
    let mut count = 0;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if byte < 48u8 || byte > 57u8 { return -1; }
        let digit = byte.to_i32_checked() - 48;
        if count > (1000000 - digit) / 10 { return -1; }
        count = count * 10 + digit;
    }
    if count == 0 { return -1; } return count;
}
fn with_paths(mode: i32, steps: i32, algorithm: i32, prefix: &[u8]) -> TensorResult<Options>{
    if prefix.len() == 0 || prefix.len() > 4093 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let mut first = new(); let mut second = new(); let a = b".a"; let b = b".b";
    if !grown(try_extend(&mut first, &*prefix)) || !grown(try_extend(&mut first, &a)) || !grown(try_extend(&mut second, &*prefix)) || !grown(try_extend(&mut second, &b)) { return TensorResult::<Options>:: Err(Error::AllocationFailed); }
    return TensorResult::Ok(Options { mode: mode, steps: steps, algorithm: algorithm, first: first, second: second });
}
fn algorithm_argument() -> i32 {
    let copied = argument(4);
    match copied {
        TensorResult::Ok(bytes) => {
            let sgd = b"sgd"; let adam = b"adam"; let adamw = b"adamw";
            if same(&bytes, &sgd) { return 0; } if same(&bytes, &adam) { return 1; } if same(&bytes, &adamw) { return 2; }
            return -1;
        },
        TensorResult::Err(error) => { return -1; }
    }
}
pub fn parse() -> TensorResult<Options>{
    if arg_count() < 3 || arg_count() > 5 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let mode_arg = argument(1);
    match mode_arg {
        TensorResult::Ok(mode_bytes) => {
            let train = b"train"; let resume = b"resume"; let mut mode = 0;
            if same(&mode_bytes, &train) { mode = 1; } if same(&mode_bytes, &resume) { mode = 2; }
            if mode == 0 { return TensorResult::<Options>:: Err(Error::Bounds); }
            if mode == 2 && arg_count() > 4 { return TensorResult::<Options>:: Err(Error::Bounds); }
            let prefix_arg = argument(2);
            match prefix_arg {
                TensorResult::Ok(prefix) => {
                    let mut steps = 2000;
                    if arg_count() >= 4 {
                        let count_arg = argument(3);
                        match count_arg { TensorResult::Ok(count_bytes) => { steps = parse_steps(&count_bytes); }, TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); } }
                    }
                    let mut algorithm = 0;
                    if arg_count() == 5 { algorithm = algorithm_argument(); }
                    if steps < 0 || algorithm < 0 { return TensorResult::<Options>:: Err(Error::Bounds); } return with_paths(mode, steps, algorithm, &prefix);
                },
                TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); }
    }
}
