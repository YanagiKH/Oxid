use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::try_push;
use alloc::vec::try_extend;
use alloc::vec::try_resize;
use alloc::vec::truncate;
use alloc::vec::take;

#[derive(Copy)]
struct Point { x: i32, y: i32 }

fn added(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn first(points: &[Point]) -> Point { return points[0]; }

fn main() -> i32 {
    let point = Point { x: 20, y: 1 };
    let mut points = new::<Point>();
    if !added(try_reserve(&mut points, 2)) { return -1; }
    if !added(try_push(&mut points, point)) { return -1; }
    let mut more = new::<Point>();
    if !added(try_push(&mut more, point)) { return -1; }
    if !added(try_extend(&mut points, &more)) { return -1; }
    if !added(try_resize(&mut points, 4, point)) { return -1; }
    truncate(&mut points, 2);
    if len(&points) != 2 || capacity(&points) < 2 { return -1; }
    let mut moved = take(&mut points);
    if len(&points) != 0 { return -1; }
    let snapshot = first(&moved);
    moved[0] = Point { x: 22, y: 1 };
    let changed = first(&moved);
    return snapshot.x + changed.x;
}
