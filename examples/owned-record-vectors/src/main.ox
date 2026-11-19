use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;

struct Message { id: i32, payload: ByteBuf }

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn main() -> i32 {
    let mut pending = new::<Message>();
    if !grew(try_reserve(&mut pending, 8)) { return -1; }
    if capacity(&pending) < 8 { return -2; }

    let mut payload = bytes_new();
    if !grew(bytes_push(&mut payload, 42u8)) { return -3; }
    push_reserved(&mut pending, Message { id: 42, payload: payload });
    if len(&pending) != 1 { return -4; }

    let mut batch = take(&mut pending);
    if len(&pending) != 0 || capacity(&pending) != 0 { return -5; }
    let message = pop_last(&mut batch);
    truncate(&mut batch, 0);
    return message.id;
}
