use alloc::vec::GrowStatus;
use crate::maps::new;
use crate::maps::len;
use crate::maps::get;
use crate::maps::try_insert;
use crate::maps::remove;
use crate::types::option::Option;
use crate::types::option::unwrap_or;

#[derive(Copy)]
struct Point { x: i32, y: i32 }

fn ok(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn replacements_and_removals() -> i32 {
    let mut map = new::<i32>();
    if len(&map) != 0 || unwrap_or(remove(&mut map, 0), 99) != 99 { return 1; }
    if !ok(try_insert(&mut map, 2147483647, 22)) { return 2; }
    if !ok(try_insert(&mut map, -2147483648, 20)) { return 3; }
    if !ok(try_insert(&mut map, 0, 9)) { return 4; }
    if !ok(try_insert(&mut map, 0, 42)) { return 5; }
    if len(&map) != 3 || unwrap_or(get(&map, 0), -1) != 42 { return 6; }
    if unwrap_or(remove(&mut map, 0), -1) != 42 || len(&map) != 2 { return 7; }
    if unwrap_or(get(&map, 0), -1) != -1 { return 8; }
    if unwrap_or(get(&map, -2147483648), -1) != 20 { return 9; }
    if unwrap_or(remove(&mut map, 2147483647), -1) != 22 { return 10; }
    if unwrap_or(remove(&mut map, -2147483648), -1) != 20 { return 11; }
    if len(&map) != 0 || !ok(try_insert(&mut map, 0, 42)) { return 12; }
    if unwrap_or(get(&map, 0), -1) != 42 { return 13; }
    return 0;
}

fn value_copies() -> i32 {
    let mut map = new::<Point>();
    let mut point = Point { x: 20, y: 22 };
    if !ok(try_insert(&mut map, 4, point)) { return 20; }
    point.x = 0;
    let observed = get(&map, 4);
    match observed {
        Option::Some(stored) => {
            if stored.x + stored.y != 42 || point.x != 0 { return 21; }
        },
        Option::None => { return 22; },
    }
    return 0;
}

fn failed_growth() -> i32 {
    let mut map = new::<i64>();
    let mut written = 0;
    let mut reached_limit = false;
    while written < 4096 && !reached_limit {
        let key = 0 - written;
        let status = try_insert(&mut map, key, written.to_i64());
        match status {
            GrowStatus::Ok => { written = written + 1; },
            GrowStatus::MemoryLimit => { reached_limit = true; },
            GrowStatus::CapacityOverflow => { return 30; },
            GrowStatus::AllocationFailed => { return 31; },
        }
    }
    if !reached_limit || written < 4 || len(&map) != written { return 32; }
    if unwrap_or(get(&map, 0 - written), -1i64) != -1i64 { return 33; }
    let mut index = 0;
    while index < written {
        if unwrap_or(get(&map, 0 - index), -1i64) != index.to_i64() { return 34; }
        index = index + 1;
    }
    if !ok(try_insert(&mut map, 0, 42i64)) { return 35; }
    if unwrap_or(remove(&mut map, 0), -1i64) != 42i64 { return 36; }
    if !ok(try_insert(&mut map, 0 - written, 42i64)) { return 37; }
    if len(&map) != written { return 38; }
    if unwrap_or(get(&map, 0 - written), -1i64) != 42i64 { return 39; }
    return 0;
}

fn main() -> i32 {
    let checked = replacements_and_removals();
    if checked != 0 { return checked; }
    let copied = value_copies();
    if copied != 0 { return copied; }
    return failed_growth();
}
