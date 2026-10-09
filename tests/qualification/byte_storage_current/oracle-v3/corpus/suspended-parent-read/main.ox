fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn take(a: &mut [u8], n: u8) -> i32 { return 0; } fn parent(p: &mut [u8]) -> i32 { return take(&mut *p,p[0]); }
fn main() -> i32 { let mut a = [byte(0)]; return parent(&mut a); }
