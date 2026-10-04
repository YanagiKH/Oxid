fn shared(a: &[i32; 1]) -> i32 { a[0] = 7; return a[0] + a.len(); }
fn exclusive(a: &mut [i32; 1]) -> i32 { a[0] = 7; return a[0] + a.len(); }
fn main() -> i32 { return 0; }
