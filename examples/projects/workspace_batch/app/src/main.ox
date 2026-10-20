use shared::base;
use shared::scale::double;
fn main() -> i32 {
    return double(base());
}
fn test_double() -> bool {
    return double(base()) == 10;
}
fn test_unit() -> () {
    return;
}
