fn main() -> i32 {
let mut checked = 0;
let n0 = 0;
let a0 = n0.to_u8_checked();
let mut j0_1 = 0;
while j0_1 < 1 {
let b0_1 = j0_1.to_u8_checked();
if (a0 == b0_1) != true { return -(1 + (0 * 256 + j0_1) * 6 + 0); }
if (a0 != b0_1) != false { return -(1 + (0 * 256 + j0_1) * 6 + 1); }
if (a0 < b0_1) != false { return -(1 + (0 * 256 + j0_1) * 6 + 2); }
if (a0 <= b0_1) != true { return -(1 + (0 * 256 + j0_1) * 6 + 3); }
if (a0 > b0_1) != false { return -(1 + (0 * 256 + j0_1) * 6 + 4); }
if (a0 >= b0_1) != true { return -(1 + (0 * 256 + j0_1) * 6 + 5); }
checked = checked + 6;
j0_1 = j0_1 + 1;
}
let mut j0_2 = 1;
while j0_2 < 256 {
let b0_2 = j0_2.to_u8_checked();
if (a0 == b0_2) != false { return -(1 + (0 * 256 + j0_2) * 6 + 0); }
if (a0 != b0_2) != true { return -(1 + (0 * 256 + j0_2) * 6 + 1); }
if (a0 < b0_2) != true { return -(1 + (0 * 256 + j0_2) * 6 + 2); }
if (a0 <= b0_2) != true { return -(1 + (0 * 256 + j0_2) * 6 + 3); }
if (a0 > b0_2) != false { return -(1 + (0 * 256 + j0_2) * 6 + 4); }
if (a0 >= b0_2) != false { return -(1 + (0 * 256 + j0_2) * 6 + 5); }
checked = checked + 6;
j0_2 = j0_2 + 1;
}
let n1 = 1;
let a1 = n1.to_u8_checked();
let mut j1_0 = 0;
while j1_0 < 1 {
let b1_0 = j1_0.to_u8_checked();
if (a1 == b1_0) != false { return -(1 + (1 * 256 + j1_0) * 6 + 0); }
if (a1 != b1_0) != true { return -(1 + (1 * 256 + j1_0) * 6 + 1); }
if (a1 < b1_0) != false { return -(1 + (1 * 256 + j1_0) * 6 + 2); }
if (a1 <= b1_0) != false { return -(1 + (1 * 256 + j1_0) * 6 + 3); }
if (a1 > b1_0) != true { return -(1 + (1 * 256 + j1_0) * 6 + 4); }
if (a1 >= b1_0) != true { return -(1 + (1 * 256 + j1_0) * 6 + 5); }
checked = checked + 6;
j1_0 = j1_0 + 1;
}
let mut j1_1 = 1;
while j1_1 < 2 {
let b1_1 = j1_1.to_u8_checked();
if (a1 == b1_1) != true { return -(1 + (1 * 256 + j1_1) * 6 + 0); }
if (a1 != b1_1) != false { return -(1 + (1 * 256 + j1_1) * 6 + 1); }
if (a1 < b1_1) != false { return -(1 + (1 * 256 + j1_1) * 6 + 2); }
if (a1 <= b1_1) != true { return -(1 + (1 * 256 + j1_1) * 6 + 3); }
if (a1 > b1_1) != false { return -(1 + (1 * 256 + j1_1) * 6 + 4); }
if (a1 >= b1_1) != true { return -(1 + (1 * 256 + j1_1) * 6 + 5); }
checked = checked + 6;
j1_1 = j1_1 + 1;
}
let mut j1_2 = 2;
while j1_2 < 256 {
let b1_2 = j1_2.to_u8_checked();
if (a1 == b1_2) != false { return -(1 + (1 * 256 + j1_2) * 6 + 0); }
if (a1 != b1_2) != true { return -(1 + (1 * 256 + j1_2) * 6 + 1); }
if (a1 < b1_2) != true { return -(1 + (1 * 256 + j1_2) * 6 + 2); }
if (a1 <= b1_2) != true { return -(1 + (1 * 256 + j1_2) * 6 + 3); }
if (a1 > b1_2) != false { return -(1 + (1 * 256 + j1_2) * 6 + 4); }
if (a1 >= b1_2) != false { return -(1 + (1 * 256 + j1_2) * 6 + 5); }
checked = checked + 6;
j1_2 = j1_2 + 1;
}
let n2 = 2;
let a2 = n2.to_u8_checked();
let mut j2_0 = 0;
while j2_0 < 2 {
let b2_0 = j2_0.to_u8_checked();
if (a2 == b2_0) != false { return -(1 + (2 * 256 + j2_0) * 6 + 0); }
if (a2 != b2_0) != true { return -(1 + (2 * 256 + j2_0) * 6 + 1); }
if (a2 < b2_0) != false { return -(1 + (2 * 256 + j2_0) * 6 + 2); }
if (a2 <= b2_0) != false { return -(1 + (2 * 256 + j2_0) * 6 + 3); }
if (a2 > b2_0) != true { return -(1 + (2 * 256 + j2_0) * 6 + 4); }
if (a2 >= b2_0) != true { return -(1 + (2 * 256 + j2_0) * 6 + 5); }
checked = checked + 6;
j2_0 = j2_0 + 1;
}
let mut j2_1 = 2;
while j2_1 < 3 {
let b2_1 = j2_1.to_u8_checked();
if (a2 == b2_1) != true { return -(1 + (2 * 256 + j2_1) * 6 + 0); }
if (a2 != b2_1) != false { return -(1 + (2 * 256 + j2_1) * 6 + 1); }
if (a2 < b2_1) != false { return -(1 + (2 * 256 + j2_1) * 6 + 2); }
if (a2 <= b2_1) != true { return -(1 + (2 * 256 + j2_1) * 6 + 3); }
if (a2 > b2_1) != false { return -(1 + (2 * 256 + j2_1) * 6 + 4); }
if (a2 >= b2_1) != true { return -(1 + (2 * 256 + j2_1) * 6 + 5); }
checked = checked + 6;
j2_1 = j2_1 + 1;
}
let mut j2_2 = 3;
while j2_2 < 256 {
let b2_2 = j2_2.to_u8_checked();
if (a2 == b2_2) != false { return -(1 + (2 * 256 + j2_2) * 6 + 0); }
if (a2 != b2_2) != true { return -(1 + (2 * 256 + j2_2) * 6 + 1); }
if (a2 < b2_2) != true { return -(1 + (2 * 256 + j2_2) * 6 + 2); }
if (a2 <= b2_2) != true { return -(1 + (2 * 256 + j2_2) * 6 + 3); }
if (a2 > b2_2) != false { return -(1 + (2 * 256 + j2_2) * 6 + 4); }
if (a2 >= b2_2) != false { return -(1 + (2 * 256 + j2_2) * 6 + 5); }
checked = checked + 6;
j2_2 = j2_2 + 1;
}
let n3 = 3;
let a3 = n3.to_u8_checked();
let mut j3_0 = 0;
while j3_0 < 3 {
let b3_0 = j3_0.to_u8_checked();
if (a3 == b3_0) != false { return -(1 + (3 * 256 + j3_0) * 6 + 0); }
if (a3 != b3_0) != true { return -(1 + (3 * 256 + j3_0) * 6 + 1); }
if (a3 < b3_0) != false { return -(1 + (3 * 256 + j3_0) * 6 + 2); }
if (a3 <= b3_0) != false { return -(1 + (3 * 256 + j3_0) * 6 + 3); }
if (a3 > b3_0) != true { return -(1 + (3 * 256 + j3_0) * 6 + 4); }
if (a3 >= b3_0) != true { return -(1 + (3 * 256 + j3_0) * 6 + 5); }
checked = checked + 6;
j3_0 = j3_0 + 1;
}
let mut j3_1 = 3;
while j3_1 < 4 {
let b3_1 = j3_1.to_u8_checked();
if (a3 == b3_1) != true { return -(1 + (3 * 256 + j3_1) * 6 + 0); }
if (a3 != b3_1) != false { return -(1 + (3 * 256 + j3_1) * 6 + 1); }
if (a3 < b3_1) != false { return -(1 + (3 * 256 + j3_1) * 6 + 2); }
if (a3 <= b3_1) != true { return -(1 + (3 * 256 + j3_1) * 6 + 3); }
if (a3 > b3_1) != false { return -(1 + (3 * 256 + j3_1) * 6 + 4); }
if (a3 >= b3_1) != true { return -(1 + (3 * 256 + j3_1) * 6 + 5); }
checked = checked + 6;
j3_1 = j3_1 + 1;
}
let mut j3_2 = 4;
while j3_2 < 256 {
let b3_2 = j3_2.to_u8_checked();
if (a3 == b3_2) != false { return -(1 + (3 * 256 + j3_2) * 6 + 0); }
if (a3 != b3_2) != true { return -(1 + (3 * 256 + j3_2) * 6 + 1); }
if (a3 < b3_2) != true { return -(1 + (3 * 256 + j3_2) * 6 + 2); }
if (a3 <= b3_2) != true { return -(1 + (3 * 256 + j3_2) * 6 + 3); }
if (a3 > b3_2) != false { return -(1 + (3 * 256 + j3_2) * 6 + 4); }
if (a3 >= b3_2) != false { return -(1 + (3 * 256 + j3_2) * 6 + 5); }
checked = checked + 6;
j3_2 = j3_2 + 1;
}
let n4 = 4;
let a4 = n4.to_u8_checked();
let mut j4_0 = 0;
while j4_0 < 4 {
let b4_0 = j4_0.to_u8_checked();
if (a4 == b4_0) != false { return -(1 + (4 * 256 + j4_0) * 6 + 0); }
if (a4 != b4_0) != true { return -(1 + (4 * 256 + j4_0) * 6 + 1); }
if (a4 < b4_0) != false { return -(1 + (4 * 256 + j4_0) * 6 + 2); }
if (a4 <= b4_0) != false { return -(1 + (4 * 256 + j4_0) * 6 + 3); }
if (a4 > b4_0) != true { return -(1 + (4 * 256 + j4_0) * 6 + 4); }
if (a4 >= b4_0) != true { return -(1 + (4 * 256 + j4_0) * 6 + 5); }
checked = checked + 6;
j4_0 = j4_0 + 1;
}
let mut j4_1 = 4;
while j4_1 < 5 {
let b4_1 = j4_1.to_u8_checked();
if (a4 == b4_1) != true { return -(1 + (4 * 256 + j4_1) * 6 + 0); }
if (a4 != b4_1) != false { return -(1 + (4 * 256 + j4_1) * 6 + 1); }
if (a4 < b4_1) != false { return -(1 + (4 * 256 + j4_1) * 6 + 2); }
if (a4 <= b4_1) != true { return -(1 + (4 * 256 + j4_1) * 6 + 3); }
if (a4 > b4_1) != false { return -(1 + (4 * 256 + j4_1) * 6 + 4); }
if (a4 >= b4_1) != true { return -(1 + (4 * 256 + j4_1) * 6 + 5); }
checked = checked + 6;
j4_1 = j4_1 + 1;
}
let mut j4_2 = 5;
while j4_2 < 256 {
let b4_2 = j4_2.to_u8_checked();
if (a4 == b4_2) != false { return -(1 + (4 * 256 + j4_2) * 6 + 0); }
if (a4 != b4_2) != true { return -(1 + (4 * 256 + j4_2) * 6 + 1); }
if (a4 < b4_2) != true { return -(1 + (4 * 256 + j4_2) * 6 + 2); }
if (a4 <= b4_2) != true { return -(1 + (4 * 256 + j4_2) * 6 + 3); }
if (a4 > b4_2) != false { return -(1 + (4 * 256 + j4_2) * 6 + 4); }
if (a4 >= b4_2) != false { return -(1 + (4 * 256 + j4_2) * 6 + 5); }
checked = checked + 6;
j4_2 = j4_2 + 1;
}
let n5 = 5;
let a5 = n5.to_u8_checked();
let mut j5_0 = 0;
while j5_0 < 5 {
let b5_0 = j5_0.to_u8_checked();
if (a5 == b5_0) != false { return -(1 + (5 * 256 + j5_0) * 6 + 0); }
if (a5 != b5_0) != true { return -(1 + (5 * 256 + j5_0) * 6 + 1); }
if (a5 < b5_0) != false { return -(1 + (5 * 256 + j5_0) * 6 + 2); }
if (a5 <= b5_0) != false { return -(1 + (5 * 256 + j5_0) * 6 + 3); }
if (a5 > b5_0) != true { return -(1 + (5 * 256 + j5_0) * 6 + 4); }
if (a5 >= b5_0) != true { return -(1 + (5 * 256 + j5_0) * 6 + 5); }
checked = checked + 6;
j5_0 = j5_0 + 1;
}
let mut j5_1 = 5;
while j5_1 < 6 {
let b5_1 = j5_1.to_u8_checked();
if (a5 == b5_1) != true { return -(1 + (5 * 256 + j5_1) * 6 + 0); }
if (a5 != b5_1) != false { return -(1 + (5 * 256 + j5_1) * 6 + 1); }
if (a5 < b5_1) != false { return -(1 + (5 * 256 + j5_1) * 6 + 2); }
if (a5 <= b5_1) != true { return -(1 + (5 * 256 + j5_1) * 6 + 3); }
if (a5 > b5_1) != false { return -(1 + (5 * 256 + j5_1) * 6 + 4); }
if (a5 >= b5_1) != true { return -(1 + (5 * 256 + j5_1) * 6 + 5); }
checked = checked + 6;
j5_1 = j5_1 + 1;
}
let mut j5_2 = 6;
while j5_2 < 256 {
let b5_2 = j5_2.to_u8_checked();
if (a5 == b5_2) != false { return -(1 + (5 * 256 + j5_2) * 6 + 0); }
if (a5 != b5_2) != true { return -(1 + (5 * 256 + j5_2) * 6 + 1); }
if (a5 < b5_2) != true { return -(1 + (5 * 256 + j5_2) * 6 + 2); }
if (a5 <= b5_2) != true { return -(1 + (5 * 256 + j5_2) * 6 + 3); }
if (a5 > b5_2) != false { return -(1 + (5 * 256 + j5_2) * 6 + 4); }
if (a5 >= b5_2) != false { return -(1 + (5 * 256 + j5_2) * 6 + 5); }
checked = checked + 6;
j5_2 = j5_2 + 1;
}
let n6 = 6;
let a6 = n6.to_u8_checked();
let mut j6_0 = 0;
while j6_0 < 6 {
let b6_0 = j6_0.to_u8_checked();
if (a6 == b6_0) != false { return -(1 + (6 * 256 + j6_0) * 6 + 0); }
if (a6 != b6_0) != true { return -(1 + (6 * 256 + j6_0) * 6 + 1); }
if (a6 < b6_0) != false { return -(1 + (6 * 256 + j6_0) * 6 + 2); }
if (a6 <= b6_0) != false { return -(1 + (6 * 256 + j6_0) * 6 + 3); }
if (a6 > b6_0) != true { return -(1 + (6 * 256 + j6_0) * 6 + 4); }
if (a6 >= b6_0) != true { return -(1 + (6 * 256 + j6_0) * 6 + 5); }
checked = checked + 6;
j6_0 = j6_0 + 1;
}
let mut j6_1 = 6;
while j6_1 < 7 {
let b6_1 = j6_1.to_u8_checked();
if (a6 == b6_1) != true { return -(1 + (6 * 256 + j6_1) * 6 + 0); }
if (a6 != b6_1) != false { return -(1 + (6 * 256 + j6_1) * 6 + 1); }
if (a6 < b6_1) != false { return -(1 + (6 * 256 + j6_1) * 6 + 2); }
if (a6 <= b6_1) != true { return -(1 + (6 * 256 + j6_1) * 6 + 3); }
if (a6 > b6_1) != false { return -(1 + (6 * 256 + j6_1) * 6 + 4); }
if (a6 >= b6_1) != true { return -(1 + (6 * 256 + j6_1) * 6 + 5); }
checked = checked + 6;
j6_1 = j6_1 + 1;
}
let mut j6_2 = 7;
while j6_2 < 256 {
let b6_2 = j6_2.to_u8_checked();
if (a6 == b6_2) != false { return -(1 + (6 * 256 + j6_2) * 6 + 0); }
if (a6 != b6_2) != true { return -(1 + (6 * 256 + j6_2) * 6 + 1); }
if (a6 < b6_2) != true { return -(1 + (6 * 256 + j6_2) * 6 + 2); }
if (a6 <= b6_2) != true { return -(1 + (6 * 256 + j6_2) * 6 + 3); }
if (a6 > b6_2) != false { return -(1 + (6 * 256 + j6_2) * 6 + 4); }
if (a6 >= b6_2) != false { return -(1 + (6 * 256 + j6_2) * 6 + 5); }
checked = checked + 6;
j6_2 = j6_2 + 1;
}
let n7 = 7;
let a7 = n7.to_u8_checked();
let mut j7_0 = 0;
while j7_0 < 7 {
let b7_0 = j7_0.to_u8_checked();
if (a7 == b7_0) != false { return -(1 + (7 * 256 + j7_0) * 6 + 0); }
if (a7 != b7_0) != true { return -(1 + (7 * 256 + j7_0) * 6 + 1); }
if (a7 < b7_0) != false { return -(1 + (7 * 256 + j7_0) * 6 + 2); }
if (a7 <= b7_0) != false { return -(1 + (7 * 256 + j7_0) * 6 + 3); }
if (a7 > b7_0) != true { return -(1 + (7 * 256 + j7_0) * 6 + 4); }
if (a7 >= b7_0) != true { return -(1 + (7 * 256 + j7_0) * 6 + 5); }
checked = checked + 6;
j7_0 = j7_0 + 1;
}
let mut j7_1 = 7;
while j7_1 < 8 {
let b7_1 = j7_1.to_u8_checked();
if (a7 == b7_1) != true { return -(1 + (7 * 256 + j7_1) * 6 + 0); }
if (a7 != b7_1) != false { return -(1 + (7 * 256 + j7_1) * 6 + 1); }
if (a7 < b7_1) != false { return -(1 + (7 * 256 + j7_1) * 6 + 2); }
if (a7 <= b7_1) != true { return -(1 + (7 * 256 + j7_1) * 6 + 3); }
if (a7 > b7_1) != false { return -(1 + (7 * 256 + j7_1) * 6 + 4); }
if (a7 >= b7_1) != true { return -(1 + (7 * 256 + j7_1) * 6 + 5); }
checked = checked + 6;
j7_1 = j7_1 + 1;
}
let mut j7_2 = 8;
while j7_2 < 256 {
let b7_2 = j7_2.to_u8_checked();
if (a7 == b7_2) != false { return -(1 + (7 * 256 + j7_2) * 6 + 0); }
if (a7 != b7_2) != true { return -(1 + (7 * 256 + j7_2) * 6 + 1); }
if (a7 < b7_2) != true { return -(1 + (7 * 256 + j7_2) * 6 + 2); }
if (a7 <= b7_2) != true { return -(1 + (7 * 256 + j7_2) * 6 + 3); }
if (a7 > b7_2) != false { return -(1 + (7 * 256 + j7_2) * 6 + 4); }
if (a7 >= b7_2) != false { return -(1 + (7 * 256 + j7_2) * 6 + 5); }
checked = checked + 6;
j7_2 = j7_2 + 1;
}
return checked;
}
