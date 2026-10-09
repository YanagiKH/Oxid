fn take(a: &[()]) -> i32 { return a.len(); }
fn main() -> i32 { let a: [u8; 0] = []; return take(&a); }
