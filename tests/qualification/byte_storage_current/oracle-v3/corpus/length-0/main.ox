fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn length(a: &[u8]) -> i32 { return a.len(); }
fn main() -> i32 { let a: [u8; 0] = []; return length(&a); }
