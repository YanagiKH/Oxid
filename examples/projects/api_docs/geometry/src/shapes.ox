/// Shape measurements.
pub mod metrics;
/// Draft API with no public items yet.
pub mod draft;
mod hidden;

/// A circle around the root point.
pub struct Circle {
    pub radius: i32,
    center: crate::Point,
}

/// Builds a circle at the origin.
pub fn make_circle(radius: i32) -> Circle {
    return Circle { radius: radius, center: crate::Point { x: 0, y: 0 } };
}

/// Returns the center through an absolute path.
pub fn center(c: &Circle) -> crate::Point {
    return crate::Point { x: c.center.x, y: c.center.y };
}

/// Same spelling as `units::area`; only this one is documented.
pub fn area(c: &Circle) -> i32 {
    return c.radius * c.radius * 3;
}
