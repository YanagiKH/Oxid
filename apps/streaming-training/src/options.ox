use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::truncate;
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
use crate::tensor::TensorResult;
use crate::tensor::Error;
pub struct Options { pub path: ByteBuf, pub epochs: i32, pub size: i32, pub start_row: i32 }
fn argument(index: i32) -> TensorResult<ByteBuf>{
    let mut bytes = new(); let grown = try_resize(&mut bytes, 4095, 0u8);
    match grown { GrowStatus::Ok => {}, GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); }, GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); }, }
    let copied = arg_bytes(index, &mut bytes); match copied { ArgStatus::Copied(length) => { truncate(&mut bytes, length); return TensorResult::Ok(bytes); }, ArgStatus::Missing => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, ArgStatus::TooSmall(required) => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, ArgStatus::TooLong => { return TensorResult::<ByteBuf>:: Err(Error::Bounds); }, }
}
fn decimal(bytes: &[u8], maximum: i32) -> i32 { if bytes.len() == 0 { return -1; } let mut value = 0; for index in 0..bytes.len() { let byte = bytes[index]; if byte < 48u8 || byte > 57u8 { return -1; } let digit = byte.to_i32_checked() - 48; if value > (maximum - digit) / 10 { return -1; } value = value * 10 + digit; } return value; }
fn number(index: i32, maximum: i32) -> i32 { let parsed = argument(index); match parsed { TensorResult::Ok(bytes) => { return decimal(&bytes, maximum); }, TensorResult::Err(error) => { return -1; }, } }
fn same(left: &[u8], right: &[u8]) -> bool { if left.len() != right.len() { return false; } for index in 0..left.len() { if left[index] != right[index] { return false; } } return true; }
pub fn help_requested() -> bool { if arg_count() == 1 { return true; } if arg_count() != 2 { return false; } let copied = argument(1); match copied { TensorResult::Ok(bytes) => { let long = b"--help"; let short = b"-h"; return same(&bytes, &long) || same(&bytes, &short); }, TensorResult::Err(error) => { return false; }, } }
pub fn parse() -> TensorResult<Options>{
    if arg_count() != 4 && arg_count() != 5 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let epochs = number(2, 1000000); let size = number(3, 2147483647);
    let mut start_row = 0;
    if arg_count() == 5 { start_row = number(4, 2147483647); }
    if epochs < 1 || size < 1 || start_row < 0 { return TensorResult::<Options>:: Err(Error::Bounds); }
    let copied = argument(1); match copied { TensorResult::Ok(path) => { return TensorResult::Ok(Options { path: path, epochs: epochs, size: size, start_row: start_row }); }, TensorResult::Err(error) => { return TensorResult::<Options>:: Err(error); }, }
}
