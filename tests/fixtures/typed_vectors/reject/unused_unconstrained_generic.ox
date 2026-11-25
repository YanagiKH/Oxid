use alloc::vec::new;
use alloc::vec::try_push;

fn invalid<T>(value: T) -> () {
    let mut values = new::<T>();
    try_push(&mut values, value);
    return;
}

fn main() -> i32 { return 0; }
