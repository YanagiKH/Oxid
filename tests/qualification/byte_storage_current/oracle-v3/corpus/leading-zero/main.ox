fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let a: [u8; 0001] = [byte(255)]; let b = a[0]; return b.to_i32(); }
