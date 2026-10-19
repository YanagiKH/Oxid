mod child;
use helper::seven;
use helper::nothing;
fn main() -> i32 {
    return seven();
}
fn test_keep_z() -> bool {
    return seven() == 7;
}
fn test_drop_false() -> bool {
    return seven() == 8;
}
fn test_keep_a() -> i32 {
    return nothing();
}
fn test_drop_error() -> i32 {
    return seven() / nothing();
}
fn test_keep_unit() -> () {
    return;
}
