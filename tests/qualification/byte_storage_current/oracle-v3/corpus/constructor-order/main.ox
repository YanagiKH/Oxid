fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn next(a: &mut [i32; 1]) -> u8 { a[0] = a[0] + 1; let x = a[0]; return x.to_u8_checked(); }
fn main() -> i32 { let mut count = [0]; let a = [next(&mut count),next(&mut count),next(&mut count)]; let x = a[0]; let y = a[1]; let z = a[2]; return x.to_i32() * 1000 + y.to_i32() * 100 + z.to_i32() * 10 + count[0]; }
