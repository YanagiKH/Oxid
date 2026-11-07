use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;

enum Payload { Empty, Bytes(ByteBuf), Wide(i128) }
struct Message { value: i32, payload: Payload }
fn make(value: i32) -> Message {
    if value == 41 {
        return Message { value: value, payload: Payload::Wide(170141183460469231731687303715884105727i128) };
    }
    let mut bytes = bytes_new();
    bytes_push(&mut bytes, 42u8);
    return Message { value: value, payload: Payload::Bytes(bytes) };
}
fn read_and_drop(message: Message) -> i32 {
    let Message { value: number, payload: payload } = message;
    match payload {
        Payload::Empty => { return -1; },
        Payload::Bytes(bytes) => { if bytes_len(&bytes) != 1 { return -2; } return number; },
        Payload::Wide(wide) => {
            if wide != 170141183460469231731687303715884105727i128 { return -3; }
            return number;
        },
    }
}
fn main() -> i32 {
    let mut values = new::<Message>();
    try_reserve(&mut values, 8);
    let reserved = capacity(&values);
    push_reserved(&mut values, make(41));
    let length = len(&values);
    let wide_value = read_and_drop(pop_last(&mut values));
    push_reserved(&mut values, make(42));
    let bytes_value = read_and_drop(pop_last(&mut values));
    push_reserved(&mut values, make(42));
    let mut taken = take(&mut values);
    truncate(&mut taken, 0);
    return wide_value + bytes_value + reserved + length + capacity(&taken) + capacity(&values);
}
