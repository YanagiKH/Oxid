use model::base;
use model::nothing;
fn main() -> i32 {
    return 0;
}
fn test_gate_first() -> bool {
    return base() == 7;
}
fn test_gate_false() -> bool {
    return base() == 70;
}
fn test_gate_divide() -> i32 {
    return base() / nothing();
}
fn test_gate_after() -> () {
    return;
}
