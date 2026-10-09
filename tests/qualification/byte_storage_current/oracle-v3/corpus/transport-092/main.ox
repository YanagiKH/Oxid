fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn moved(a: [u8; 1]) -> [u8; 1] { return a; }
fn exact(a: &[u8; 1]) -> i32 { let b = a[0]; return b.to_i32(); }
fn slice(a: &[u8]) -> i32 { let b = a[0]; return b.to_i32(); }
fn relay(a: &mut [u8]) -> i32 { return slice(&*a); }
fn write(a: &mut [u8], b: u8) -> () { a[0] = b; return; }
fn main() -> i32 {
 let original = [byte(92)];
 let returned = moved(original);
 let mut a = returned;
 let b = a[0];
 if b.to_i32() != 92 { return -1; }
 if exact(&a) != 92 { return -2; }
 if slice(&a) != 92 { return -3; }
 if relay(&mut a) != 92 { return -4; }
 write(&mut a, byte(163));
 if slice(&a) != 163 { return -5; }
 a = [byte(92)];
 if exact(&a) != 92 { return -6; }
 return 92;
}
