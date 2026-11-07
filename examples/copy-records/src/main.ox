#[derive(Copy)]
struct Point {
    x: i32,
    y: i32,
}

fn total(point: Point) -> i32 {
    return point.x + point.y;
}

fn main() -> i32 {
    let first = Point { x: 20, y: 1 };
    let second = first;
    return total(first) + total(second);
}
