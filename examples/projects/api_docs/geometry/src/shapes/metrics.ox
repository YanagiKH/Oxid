use crate::shapes::Circle as Round;
use units::Meters;
use units::meters;

/// Diameter of `c`.
pub fn diameter(c: &Round) -> i32 {
    return c.radius * 2;
}

pub fn span(m: Meters) -> Meters {
    return meters(m.value * 2);
}

/// Scales the first length.
pub fn scaled(lengths: &mut [i32; 2], by: i32) -> () {
    lengths[0] = lengths[0] * by;
    return;
}
