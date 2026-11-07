use alloc::bytes::ByteBuf;
use alloc::bytes::new;
pub struct Message { pub value: i32, pub bytes: ByteBuf }
pub fn make() -> Message { return Message { value: 42, bytes: new() }; }
