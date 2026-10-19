mod child;
use helper::seven;
use helper::nothing;
fn main() -> i32 {
    return seven();
}
fn test_early_pass() -> bool {
    return seven() == 7;
}
fn test_early_false() -> bool {
    return seven() == 8;
}
fn test_mid_pass() -> i32 {
    return nothing();
}
fn test_late_divide() -> i32 {
    return seven() / nothing();
}
fn test_late_unit() -> () {
    return;
}
