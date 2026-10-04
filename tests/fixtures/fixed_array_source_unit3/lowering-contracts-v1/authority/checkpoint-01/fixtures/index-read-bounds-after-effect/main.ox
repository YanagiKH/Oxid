fn touch(a: &mut [i32; 1]) -> i32 { a[0] = 99; return a[1]; }
fn main() -> i32 { let mut a = [5]; a[touch(&mut a)] = a[0]; return a[0]; }
