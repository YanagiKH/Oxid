use helper::seven;
use helper::nothing;
fn main() -> i32 {
    return nothing();
}
fn test_green_bool() -> bool {
    return seven() == 7;
}
fn test_green_zero() -> i32 {
    return nothing();
}
fn test_green_unit() -> () {
    return;
}
