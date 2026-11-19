use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::vec::new;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved as store;
use alloc::vec::pop_last as remove;
struct Message { value: i32, bytes: ByteBuf }
fn main() -> i32 {
    let mut values = new::<Message>();
    try_reserve(&mut values, 1);
    store(&mut values, Message { value: 42, bytes: bytes_new() });
    let result = remove(&mut values);
    return result.value;
}
