fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn child(a: &mut [u8]) -> () { a[0] = byte(255); return; }
fn parent(a: &mut [u8; 1]) -> i32 { child(&mut *a); let b = a[0]; return b.to_i32(); }
fn main() -> i32 { let mut a = [byte(128)]; return parent(&mut a); }
