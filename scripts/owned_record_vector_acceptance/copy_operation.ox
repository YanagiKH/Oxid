use alloc::vec::new;
use alloc::vec::push_reserved;
fn main() -> i32 {
    let mut values = new::<i32>();
    push_reserved(&mut values, 1);
    return 0;
}
