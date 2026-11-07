use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::vec::new;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
struct Message<T> { value: T, bytes: ByteBuf }
fn identity<T>(value: T) -> T { return value; }
fn main() -> i32 {
    let mut values = new::<Message<i32>>();
    try_reserve(&mut values, 1);
    push_reserved(&mut values, identity(Message::<i32> { value: 42, bytes: bytes_new() }));
    let result = pop_last(&mut values);
    return result.value;
}
