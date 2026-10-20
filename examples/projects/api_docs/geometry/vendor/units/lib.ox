/// Units point: same spelling as the geometry root `Point`.
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub struct Meters {
    pub value: i32,
}

pub enum Axis {
    Horizontal,
    Vertical,
}

pub fn origin() -> Point {
    return Point { x: 0, y: 0 };
}

pub fn meters(value: i32) -> Meters {
    return Meters { value: value };
}

/// Same spelling as `geometry::shapes::area`.
pub fn area(p: &Point) -> i32 {
    return p.x * p.y;
}

fn hidden() -> i32 {
    return 1;
}
