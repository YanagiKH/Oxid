fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn take(a: &mut [u8]) -> i32 { return a.len(); } fn parent(p: &[u8]) -> i32 { return take(&mut *p); }
fn main() -> i32 { let a = [byte(0)]; return parent(&a); }
