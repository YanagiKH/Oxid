use alloc::bytes::ByteBuf;
use alloc::vec::new;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
struct Message { bytes: ByteBuf }
fn main() -> i32 {
    let mut values = new::<Message>();
    push_reserved(&mut values, pop_last(&mut values));
    return 0;
}
