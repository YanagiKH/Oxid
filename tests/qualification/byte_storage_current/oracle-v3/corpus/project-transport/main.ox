mod helper;
use crate::helper::read as read;
fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 { let a = [byte(255)]; return read(&a); }
