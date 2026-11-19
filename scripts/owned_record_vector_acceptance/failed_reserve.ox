use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
struct Message { value: i32, bytes: ByteBuf }
fn make(value: i32) -> Message {
    let mut bytes = bytes_new();
    bytes_push(&mut bytes, value.to_u8_checked());
    return Message { value: value, bytes: bytes };
}
fn read_and_drop(message: Message) -> i32 {
    if bytes_len(&message.bytes) != 1 { return -10; }
    return message.value;
}
fn main() -> i32 {
    let mut values = new::<Message>();
    let initial = try_reserve(&mut values, 2);
    match initial {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return -1; },
        GrowStatus::AllocationFailed => { return -2; },
        GrowStatus::MemoryLimit => { return -3; },
    }
    push_reserved(&mut values, make(7));
    let incoming = make(9);
    let old_length = len(&values);
    let old_capacity = capacity(&values);
    let failed = try_reserve(&mut values, 64);
    match failed {
        GrowStatus::MemoryLimit => {},
        GrowStatus::Ok => { return -4; },
        GrowStatus::CapacityOverflow => { return -5; },
        GrowStatus::AllocationFailed => { return -6; },
    }
    if len(&values) != old_length || capacity(&values) != old_capacity { return -7; }
    let overflow = try_reserve(&mut values, 2147483647);
    match overflow {
        GrowStatus::CapacityOverflow => {},
        GrowStatus::Ok => { return -8; },
        GrowStatus::MemoryLimit => { return -9; },
        GrowStatus::AllocationFailed => { return -11; },
    }
    if len(&values) != old_length || capacity(&values) != old_capacity { return -12; }
    push_reserved(&mut values, incoming);
    let last = read_and_drop(pop_last(&mut values));
    let first = read_and_drop(pop_last(&mut values));
    return first * 10 + last;
}
