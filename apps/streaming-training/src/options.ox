use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::len;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::truncate;
use alloc::bytes::try_extend;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use crate::tensor::TensorResult;
use crate::tensor::Error;
pub struct Options {
    pub path: ByteBuf,
    pub epochs: i32,
    pub size: i32,
    pub start_row: i32,
    pub shuffle: bool,
    pub seed: u32,
    pub checkpoint_mode: i32,
    pub first: ByteBuf,
    pub second: ByteBuf,
}
fn argument(index: i32) -> TensorResult<ByteBuf>{
    let mut bytes = new(); let grown = try_resize(&mut bytes, 4095, 0u8);
    match grown { GrowStatus::Ok => {}, GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); }, GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); }, }
    let copied = arg_bytes(index, &mut bytes); match copied { ArgStatus::Copied(length) => { truncate(&mut bytes, length); return TensorResult::Ok(bytes); }, ArgStatus::Missing => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, ArgStatus::TooSmall(required) => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, ArgStatus::TooLong => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, }
}
fn decimal(bytes: &[u8], maximum: i64) -> i64 {
    if bytes.len() == 0 { return -1i64; }
    let mut value = 0i64;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if byte < 48u8 || byte > 57u8 { return -1i64; }
        let digit = byte.to_i64_checked() - 48i64;
        if value > (maximum - digit) / 10i64 { return -1i64; }
        value = value * 10i64 + digit;
    }
    return value;
}
fn number(index: i32, maximum: i64) -> i64 {
    let parsed = argument(index);
    match parsed {
        TensorResult::Ok(bytes) => { return decimal(&bytes, maximum); },
        TensorResult::Err(error) => { return -1i64; },
    }
}
fn same(left: &[u8], right: &[u8]) -> bool { if left.len() != right.len() { return false; } for index in 0..left.len() { if left[index] != right[index] { return false; } } return true; }
pub fn help_requested() -> bool { if arg_count() == 1 { return true; } if arg_count() != 2 { return false; } let copied = argument(1); match copied { TensorResult::Ok(bytes) => { let long = b"--help"; let short = b"-h"; return same(&bytes, &long) || same(&bytes, &short); }, TensorResult::Err(error) => { return false; }, } }
fn shuffle_seed(index: i32) -> i64 {
    let flag = argument(index);
    match flag {
        TensorResult::Ok(bytes) => {
            let wanted = b"--shuffle";
            if same(&bytes, &wanted) { return number(index + 1, 4294967295i64); }
        },
        TensorResult::Err(error) => {},
    }
    return -1i64;
}

fn checkpoint_flag(index: i32) -> i32 {
    let flag = argument(index);
    match flag {
        TensorResult::Ok(bytes) => {
            let save = b"--checkpoint";
            let resume = b"--resume";
            if same(&bytes, &save) { return 1; }
            if same(&bytes, &resume) { return 2; }
        },
        TensorResult::Err(error) => {},
    }
    return 0;
}

fn extend(path: &mut ByteBuf, text: &[u8]) -> TensorResult<()> {
    let grown = try_extend(&mut *path, &*text);
    match grown {
        GrowStatus::Ok => { return TensorResult::Ok(()); },
        GrowStatus::CapacityOverflow => { return TensorResult::<()>::Err(Error::SizeOverflow); },
        GrowStatus::MemoryLimit => { return TensorResult::<()>::Err(Error::MemoryLimit); },
        GrowStatus::AllocationFailed => { return TensorResult::<()>::Err(Error::AllocationFailed); },
    }
}

fn checkpoint_paths(options: Options, index: i32) -> TensorResult<Options> {
    let prefix_arg = argument(index);
    match prefix_arg {
        TensorResult::Ok(prefix) => {
            let size = len(&prefix);
            if size == 0 || size > 4093 { return TensorResult::<Options>::Err(Error::Bounds); }
            let mut configured = options;
            let first_suffix = b".a";
            let second_suffix = b".b";
            let first = extend(&mut configured.first, &prefix);
            match first { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Options>::Err(error); }, }
            let first_end = extend(&mut configured.first, &first_suffix);
            match first_end { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Options>::Err(error); }, }
            let second = extend(&mut configured.second, &prefix);
            match second { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Options>::Err(error); }, }
            let second_end = extend(&mut configured.second, &second_suffix);
            match second_end { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Options>::Err(error); }, }
            return TensorResult::Ok(configured);
        },
        TensorResult::Err(error) => { return TensorResult::<Options>::Err(error); },
    }
}

pub fn parse() -> TensorResult<Options>{
    let argc = arg_count();
    if argc < 4 || argc > 9 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let mut positional = argc;
    let mut mode = 0;
    if argc >= 6 {
        mode = checkpoint_flag(argc - 2);
        if mode != 0 { positional = positional - 2; }
    }
    if positional > 7 { return TensorResult::<Options>::Err(Error::Bounds); }
    let shuffle = positional >= 6;
    let mut seed = 0u32;
    if shuffle {
        positional = positional - 2;
        let parsed_seed = shuffle_seed(positional);
        if parsed_seed < 0i64 { return TensorResult::<Options>:: Err(Error::Bounds); }
        seed = parsed_seed.to_u32_checked();
    }

    let epochs = number(2, 1000000i64);
    let size = number(3, 2147483647i64);
    let mut start_row = 0i64;
    if positional == 5 { start_row = number(4, 2147483647i64); }
    if epochs < 1i64 || size < 1i64 || start_row < 0i64 {
        return TensorResult::<Options>:: Err(Error::Bounds);
    }

    let copied = argument(1);
    match copied {
        TensorResult::Ok(path) => {
            let options = Options {
                path: path, epochs: epochs.to_i32_checked(), size: size.to_i32_checked(),
                start_row: start_row.to_i32_checked(), shuffle: shuffle, seed: seed,
                checkpoint_mode: mode, first: new(), second: new(),
            };
            if mode != 0 { return checkpoint_paths(options, argc - 1); }
            return TensorResult::Ok(options);
        },
        TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); },
    }
}
