fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let a = [byte(0)]; return a[0].to_i32(); }
