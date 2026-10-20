mod tally;
use measure::unit;
use kernel::seed;
use kernel::scale::double;

pub fn summary() -> i32 {
    let s = seed();
    return double(s.value) + unit() + crate::tally::count();
}
