fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn take(a: &[u8], b: &mut [u8]) -> i32 { return 0; }
fn main() -> i32 { let mut a = [byte(0)]; return take(&a,&mut a); }
