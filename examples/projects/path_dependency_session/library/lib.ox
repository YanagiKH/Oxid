pub mod model;
use shared::number;
fn local()->i32 { return number(); }
pub fn answer()->i32 { return crate::local(); }
fn main()->i32 { return 99; }
fn test_dependency(x:i32)->i32 { return x; }
