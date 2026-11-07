use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;

struct Message { value: i32, bytes: ByteBuf }
fn identity<T>(value: T) -> T { return value; }
fn make(value: i32) -> Message {
    let mut bytes = bytes_new();
    bytes_push(&mut bytes, value.to_u8_checked());
    return Message { value: value, bytes: bytes };
}
fn read_and_drop(message: Message) -> i32 { return message.value; }
fn main() -> i32 {
    let mut values = new::<Message>();
    try_reserve(&mut values, 8);
    let reserved = capacity(&values);
    push_reserved(&mut values, identity(make(41)));
    let length = len(&values);
    let number = read_and_drop(pop_last(&mut values));
    push_reserved(&mut values, make(42));
    let mut taken = take(&mut values);
    truncate(&mut taken, 0);
    return number + reserved + length + capacity(&taken) + capacity(&values);
}
