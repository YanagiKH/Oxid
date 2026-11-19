use alloc::bytes::GrowStatus;
use crate::vector::Read;
use crate::vector::new;
use crate::vector::len;
use crate::vector::try_push;
use crate::vector::get;

fn main() -> f64 {
    let mut values = new();
    let input = [1.5, 2.5, 8.0];
    let mut index = 0;
    while index < input.len() {
        let appended = try_push(&mut values, input[index]);
        match appended {
            GrowStatus::Ok => {},
            GrowStatus::CapacityOverflow => { return -1.0; },
            GrowStatus::AllocationFailed => { return -1.0; },
            GrowStatus::MemoryLimit => { return -1.0; },
        }
        index = index + 1;
    }
    let mut total = 0.0;
    index = 0;
    while index < len(&values) {
        let current = get(&values, index);
        match current {
            Read::Value(value) => { total = total + value; },
            Read::OutOfBounds => { return -1.0; },
        }
        index = index + 1;
    }
    let count = len(&values);
    return total / count.to_f64();
}
