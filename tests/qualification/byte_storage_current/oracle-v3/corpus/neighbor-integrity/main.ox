fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let mut a = [byte(128),byte(255)]; let z: [u8; 0] = []; let b = [byte(127),byte(1)]; a[1] = byte(2); let x = a[0]; let y = b[0]; let q = b[1]; return x.to_i32() + y.to_i32() + q.to_i32() + z.len(); }
