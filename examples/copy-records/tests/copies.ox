#[derive(Copy)]
struct Point { x: i32, y: i32 }

#[derive(Copy)]
struct Holder<T: Copy> { value: T }

fn identity<T: Copy>(value: T) -> T { return value; }
fn inspect(first: &Point, second: Point) -> i32 { return first.x + second.x; }

fn main() -> i32 {
    let mut first = Point { x: 20, y: 1 };
    let second = identity(first);
    first.x = 22;
    if second.x != 20 { return 1; }
    if inspect(&first, first) != 44 { return 2; }
    let holder = Holder::<Point> { value: second };
    let duplicate = holder;
    let Holder { value: extracted } = duplicate;
    let Holder { value: original } = holder;
    if extracted.x + original.x != 40 { return 3; }
    if second.y != 1 { return 4; }
    return 0;
}
