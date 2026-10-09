fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn index(a: &mut [u8]) -> i32 { a[0] = byte(255); return 0; }
fn main() -> i32 { let mut a = [byte(128)]; a[index(&mut a)] = a[0]; let b = a[0]; return b.to_i32(); }
