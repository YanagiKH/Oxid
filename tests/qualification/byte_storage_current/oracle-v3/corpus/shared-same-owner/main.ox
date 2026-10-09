fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn both(a: &[u8; 1], b: &[u8]) -> i32 { let x = a[0]; let y = b[0]; return x.to_i32() + y.to_i32(); }
fn main() -> i32 { let a = [byte(128)]; return both(&a,&a); }
