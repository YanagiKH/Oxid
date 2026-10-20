/// Shapes built on the root `Point`.
pub mod shapes;
/// Comment attachment boundaries.
///
/// Each function below shows one rule.
pub mod notes;
/// Private subtree: never documented.
mod internal;
use units::Point as UnitPoint;
use units::Meters;
use units::Axis  as  Direction;
use crate::shapes::Circle;
use units::origin;

/// A point on the plane: `x` and `y` are <i32> values.
///
/// See [the guide](https://example.com/guide?q=1&r=2) — café, 日本語, 🦀.
pub struct Point {
    pub x: i32,
    /// Field comments never attach independently.
    pub y: i32,
}

/// Scales `p` in place by `factor`.
pub fn zoom(p: &mut Point, factor: i32) -> () {
    p.x = p.x * factor;
    p.y = p.y * factor;
    return;
}

/// Account with hidden state.
pub struct Account {
    pub id: i32,
    balance: i32,
    pub tags: [i32; 3],
    distance: Meters,
}

pub fn convert(p: &UnitPoint) -> Point {
    return Point { x: p.x, y: p.y };
}

pub fn start() -> UnitPoint {
    return origin();
}

/// Every payload form.
pub enum Shape {
    Empty,
    /// Variant comments never attach independently.
    Dot(Point),
    Ring(Circle),
    Line([i32; 4]),
    Flag(bool),
    Nothing(()),
    Toward(Direction),
}

pub fn  total ( values : & /* shared */ [i32] ,
    bytes: &mut [u8], window: &[i32; 04] ) -> i32 {
    return values.len() + bytes.len() + window[0];
}

fn measure(m: Meters) -> i32 {
    return m.value;
}

struct Cache {
    hits: i32,
}

enum Mode {
    Fast,
    Slow,
}

/// Marker with no fields.
pub struct Marker {}

pub struct Sealed {
    secret: i32,
}

/// Name and alias rules.
pub mod name_rules;
