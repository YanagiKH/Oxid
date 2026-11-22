use crate::random::below;
use crate::random::seeded;
use crate::random::Draw;

fn main() -> i32 {
    let mut random = seeded(1u32);
    let draw = below(&mut random, 100u32);
    match draw {
        Draw::Value(value) => { return value.to_i32_checked(); },
        Draw::EmptyRange => { return -1; },
    }
}
