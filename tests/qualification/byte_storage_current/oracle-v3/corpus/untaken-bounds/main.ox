fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let a = [byte(128)]; if false { let b = a[-1]; return b.to_i32(); } return a.len(); }
