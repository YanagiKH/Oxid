pub mod scale;
pub fn base() -> i32 {
    return 5;
}
fn main() -> i32 {
    return base();
}
fn test_base() -> bool {
    return base() == 5;
}
