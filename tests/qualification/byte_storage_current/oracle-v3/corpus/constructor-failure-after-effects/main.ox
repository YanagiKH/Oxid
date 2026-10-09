fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn next(a: &mut [i32; 1], n: i32) -> u8 { a[0] = a[0] + 1; return n.to_u8_checked(); }
fn main() -> i32 { let mut count = [0]; let a = [next(&mut count,128),next(&mut count,256),next(&mut count,255)]; return count[0]; }
