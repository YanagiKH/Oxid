use alloc::bytes::ByteBuf;
use alloc::vec::new;
struct Message { bytes: ByteBuf }
fn main() -> i32 { new::<Message>(); return 0; }
