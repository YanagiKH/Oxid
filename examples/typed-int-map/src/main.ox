use alloc::vec::GrowStatus;
use crate::maps::new;
use crate::maps::len;
use crate::maps::get;
use crate::maps::try_insert;
use crate::maps::remove;
use crate::types::option::unwrap_or;

fn succeeded(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn main() -> i32 {
    let mut settings = new::<i32>();
    if !succeeded(try_insert(&mut settings, 9, 22)) { return -1; }
    if !succeeded(try_insert(&mut settings, -2, 20)) { return -2; }
    let selected = unwrap_or(get(&settings, -2), 0);
    let removed = unwrap_or(remove(&mut settings, 9), 0);
    if len(&settings) != 1 { return -3; }
    return selected + removed;
}
