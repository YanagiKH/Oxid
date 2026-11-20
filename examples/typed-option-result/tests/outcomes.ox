use alloc::bytes::new;
use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use alloc::bytes::try_resize;
use crate::types::option::Option;
use crate::types::option::ok_or;
use crate::types::option::unwrap_or as with_default;
use crate::types::result::Result;
use crate::types::result::unwrap_or as with_result_default;
use crate::types::result::ok;
use crate::types::result::err;

fn first_bytes(bytes: &[u8]) -> i32 {
    if bytes.len() != 2 { return -1; }
    let first = bytes[0];
    let second = bytes[1];
    return first.to_i32() + second.to_i32();
}

fn reserved_bytes() -> ByteBuf {
    let mut bytes = new();
    let grown = try_resize(&mut bytes, 1024, 42u8);
    match grown {
        GrowStatus::Ok => { return bytes; },
        GrowStatus::CapacityOverflow => { return new(); },
        GrowStatus::AllocationFailed => { return new(); },
        GrowStatus::MemoryLimit => { return new(); },
    }
}

fn allocation_round() -> bool {
    let payload = reserved_bytes();
    let error = reserved_bytes();
    let result = ok_or(Option::<ByteBuf>::Some(payload), error);
    let success = with_default(ok(result), reserved_bytes());
    if len(&success) != 1024 { return false; }
    let missing = ok_or(Option::<ByteBuf>::None, reserved_bytes());
    let failure = with_result_default(missing, reserved_bytes());
    if len(&failure) != 1024 { return false; }
    let recovered = with_default(err(Result::<i32, ByteBuf>::Err(reserved_bytes())), new());
    return len(&recovered) == 1024;
}

fn main() -> i32 {
    let number = with_default(Option::<i32>::None, 42);
    if number != 42 { return 1; }
    let number_result = with_result_default(Result::<i32, bool>::Err(true), 42);
    if number_result != 42 { return 2; }
    let code = with_default(err(ok_or(Option::<char>::None, 42)), 0);
    if code != 42 { return 3; }
    let mut owned = new();
    let payload = b" &";
    let copied = try_extend(&mut owned, &payload);
    match copied {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return 4; },
        GrowStatus::AllocationFailed => { return 5; },
        GrowStatus::MemoryLimit => { return 6; },
    }
    let selected: Result<ByteBuf, i32> = ok_or(Option::<ByteBuf>::Some(owned), 7);
    let available = ok(selected);
    let bytes = with_default(available, new());
    if first_bytes(&bytes) != 70 { return 8; }
    for round in 0..16 {
        if !allocation_round() { return 9; }
    }
    return 0;
}
