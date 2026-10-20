// Selected workspace member. Unicode survives byte for byte: café ☕ 日本語 🦀
mod report;
use geo::origin_offset;
use geo::shapes::area::square_area;
use geo::shapes::area::doubled_area;
use stats::summary;
use crate::report::frame::framed;

fn main() -> i32 {
    return framed(square_area(3) + doubled_area(1) + origin_offset() + summary());
}

fn test_area() -> bool {
    return square_area(3) + doubled_area(1) == 11;
}

fn test_distinct_units() -> bool {
    return origin_offset() * 1000 + summary() == 6117;
}

fn test_report() -> i32 {
    return framed(0) - 1000;
}
