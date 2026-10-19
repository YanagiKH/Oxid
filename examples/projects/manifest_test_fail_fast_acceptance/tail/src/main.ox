use helper::seven;
fn main() -> i32 {
    return 0;
}
fn test_tail_pass() -> bool {
    return seven() == 7;
}
fn test_tail_unit() -> () {
    return;
}
fn test_tail_nonzero() -> i32 {
    return seven() - 4;
}
