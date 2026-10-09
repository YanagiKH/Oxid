fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let a: [u8; 0] = ([]); let b = a; return b.len(); }
