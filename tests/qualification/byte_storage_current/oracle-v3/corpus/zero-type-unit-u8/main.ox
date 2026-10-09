fn take(a: &[u8]) -> i32 { return a.len(); }
fn main() -> i32 { let a: [(); 0] = []; return take(&a); }
