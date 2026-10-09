fn main() -> i32 {
let mut x = 0;
while x < 256 {
let b = x.to_u8_checked();
let wide = b.to_i32();
if wide != x { return -(x + 1); }
x = x + 1;
}
return x;
}
