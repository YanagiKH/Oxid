/// Geometry member: depends on the shared kernel and its own `units`.
pub mod shapes;
use kernel::seed;
use units::unit;

pub fn origin_offset() -> i32 {
    let s = seed();
    return s.value + unit();
}
