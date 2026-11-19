use alloc::bytes::ByteBuf;
use alloc::vec::new;
struct Message { bytes: ByteBuf }
fn main() -> i32 {
    let values = new::<Message>();
    let item = values[0];
    return 0;
}
