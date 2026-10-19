pub mod model;
fn factor() -> i32 {
    return 3;
}
pub fn scale(x: i32) -> i32 {
    return x * crate::factor();
}
fn main() -> i32 {
    return 55;
}
fn test_library() -> bool {
    return scale(1) == 3;
}
