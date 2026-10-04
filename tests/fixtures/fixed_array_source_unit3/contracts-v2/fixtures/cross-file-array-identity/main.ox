mod child;
fn main() -> i32 { let a: [i32; 1] = crate::child::make(); return crate::child::take(a); }
