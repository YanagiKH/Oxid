use geometry::Point;
use geometry::zoom;
use geometry::shapes::make_circle;
use geometry::shapes::area;

/// Never executed by documentation.
fn main() -> i32 {
    let mut p = Point { x: 2, y: 3 };
    zoom(&mut p, 2);
    let c = make_circle(p.x);
    return area(&c) + p.y;
}

/// Doubles a point built from `x`.
pub fn doubled(x: i32) -> Point {
    let mut p = Point { x: x, y: x };
    zoom(&mut p, 2);
    return p;
}

/// Same spelling as `geometry::shapes::area`.
pub fn area_of(radius: i32) -> i32 {
    let c = make_circle(radius);
    return area(&c);
}
