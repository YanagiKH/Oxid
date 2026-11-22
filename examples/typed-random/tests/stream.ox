use crate::random::below;
use crate::random::next_nonzero;
use crate::random::seeded;
use crate::random::Draw;

fn expect(draw: Draw, expected: u32) -> bool {
    match draw {
        Draw::Value(actual) => { return actual == expected; },
        Draw::EmptyRange => { return false; },
    }
}

fn main() -> i32 {
    let mut first = seeded(1u32);
    let invalid = below(&mut first, 0u32);
    match invalid {
        Draw::Value(value) => { return 1; },
        Draw::EmptyRange => {},
    }
    if next_nonzero(&mut first) != 270369u32 { return 2; }
    if next_nonzero(&mut first) != 67634689u32 { return 3; }
    if next_nonzero(&mut first) != 2647435461u32 { return 4; }

    let mut zero = seeded(0u32);
    if next_nonzero(&mut zero) != 1085196063u32 { return 5; }

    let mut last = seeded(4294967295u32);
    if !expect(below(&mut last, 4294967295u32), 253982u32) { return 6; }
    if !expect(below(&mut last, 1u32), 0u32) { return 7; }

    let mut retry = seeded(8225u32);
    if !expect(below(&mut retry, 2147483648u32), 1650780473u32) { return 8; }
    if next_nonzero(&mut retry) != 389613051u32 { return 9; }
    return 0;
}
