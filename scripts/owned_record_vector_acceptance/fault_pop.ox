use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::vec::new;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
struct Message { bytes: ByteBuf }
fn main() -> i32 {
    let mut values = new::<Message>();
    try_reserve(&mut values, 1);
    let mut bytes = bytes_new();
    bytes_push(&mut bytes, 7u8);
    push_reserved(&mut values, Message { bytes: bytes });
    let live = pop_last(&mut values);
    let missing = pop_last(&mut values);
    return 99;
}
