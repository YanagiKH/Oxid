use crate::model::Message;
use crate::model::make;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;
fn main() -> i32 {
    let mut values = new::<Message>();
    try_reserve(&mut values, 2);
    push_reserved(&mut values, make());
    if len(&values) != 1 || capacity(&values) < 2 { return -1; }
    let mut batch = take(&mut values);
    let message = pop_last(&mut batch);
    truncate(&mut batch, 0);
    if len(&values) != 0 || capacity(&values) != 0 { return -2; }
    return message.value;
}
