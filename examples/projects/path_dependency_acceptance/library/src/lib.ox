pub mod model;
use shared::base;
fn bonus() -> i32 {
    return 3;
}
pub fn total() -> i32 {
    return base() + crate::bonus();
}
fn main() -> i32 {
    return 99;
}
fn test_library(x: i32) -> i32 {
    return x;
}
