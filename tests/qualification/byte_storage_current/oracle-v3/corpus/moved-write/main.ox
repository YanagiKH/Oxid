fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let mut a = [byte(0)]; let b = a; a[0] = byte(1); return 0; }
