use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::try_push;
use alloc::vec::try_extend;

fn grown(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn main() -> i32 {
    let mut values = new::<i32>();
    if !grown(try_push(&mut values, 20)) { return -1; }
    let more = [22];
    if !grown(try_extend(&mut values, &more)) { return -1; }
    return values[0] + values[1];
}
