use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
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
    let mut messages = new::<Message>();
    if !grew(try_reserve(&mut messages, 2)) { return 1; }
    let mut first = bytes_new();
    if !grew(bytes_push(&mut first, 7u8)) { return 2; }
    push_reserved(&mut messages, Message { id: 7, payload: first });
    push_reserved(&mut messages, Message { id: 9, payload: bytes_new() });
    let last = pop_last(&mut messages);
    if last.id != 9 || bytes_len(&last.payload) != 0 { return 3; }
    let mut remaining = take(&mut messages);
    if len(&messages) != 0 || len(&remaining) != 1 { return 4; }
    let saved = pop_last(&mut remaining);
    if saved.id != 7 || bytes_len(&saved.payload) != 1 { return 5; }
    truncate(&mut remaining, 0);
    return 0;
}
