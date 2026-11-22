use alloc::bytes::ByteBuf;
use alloc::vec::Vec;
use alloc::vec::new;
struct Message { bytes: ByteBuf }
struct Holder { values: Vec<Message> }
fn main() -> i32 {
    let nested = new::<Holder>();
    return 0;
}
