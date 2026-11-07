use alloc::vec::new;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use crate::text::Text;
use crate::text::new_text;
use crate::text::byte_len;

fn main() -> i32 {
    let mut texts = new::<Text>();
    try_reserve(&mut texts, 1);
    let text = new_text();
    push_reserved(&mut texts, text);
    return byte_len(&text);
}
