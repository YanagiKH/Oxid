use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;
use alloc::vec::try_extend;

#[derive(Copy)]
struct Point { x: i32, y: i32 }

fn first<T: Copy>(values: &[T]) -> T { return values[0]; }
fn replace<T: Copy>(values: &mut [T], value: T) { values[0] = value; }

fn main() -> i32 {
    let point = Point { x: 20, y: 1 };
    let mut values = new::<Point>();
    try_push(&mut values, point);
    let snapshot = first(&values);
    let replacement = Point { x: 22, y: 1 };
    replace(&mut values[0..1], replacement);
    let changed = first(&values);
    if snapshot.x != 20 || point.x != 20 { return 1; }
    if changed.x != 22 || replacement.x != 22 { return 2; }
    let mut copied = new::<Point>();
    try_extend(&mut copied, &values[0..1]);
    if len(&copied) != 1 { return 3; }
    let copied_first = first(&copied);
    if copied_first.x != 22 || copied_first.y != 1 { return 4; }
    return 0;
}
