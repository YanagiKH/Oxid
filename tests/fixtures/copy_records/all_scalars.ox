#[derive(Copy)]
struct Scalars {
    a: i8, b: i16, c: i32, d: i64, e: i128, f: isize,
    g: u8, h: u16, i: u32, j: u64, k: u128, l: usize,
    m: f32, n: f64, o: bool, p: char, q: (),
}
fn main() -> i32 {
    let nan = f64::from_bits(9221120237041090602u64);
    let first = Scalars {
        a: -128i8, b: -32768i16, c: -2147483648,
        d: -9223372036854775808i64,
        e: -170141183460469231731687303715884105728i128,
        f: -9223372036854775808isize,
        g: 255u8, h: 65535u16, i: 4294967295u32,
        j: 18446744073709551615u64,
        k: 340282366920938463463374607431768211455u128,
        l: 18446744073709551615usize,
        m: -0.0f32, n: nan, o: true, p: '🦀', q: (),
    };
    let second = first;
    if first.a != second.a || first.b != second.b || first.c != second.c { return 1; }
    if first.d != second.d || first.e != second.e || first.f != second.f { return 2; }
    if first.g != second.g || first.h != second.h || first.i != second.i { return 3; }
    if first.j != second.j || first.k != second.k || first.l != second.l { return 4; }
    let first_zero = first.m;
    let second_zero = second.m;
    if first_zero.to_bits() != 2147483648u32 || second_zero.to_bits() != 2147483648u32 { return 5; }
    let first_nan = first.n;
    let second_nan = second.n;
    if first_nan.to_bits() != 9221120237041090602u64 || second_nan.to_bits() != 9221120237041090602u64 { return 6; }
    if first.o != second.o || first.p != second.p { return 7; }
    let first_unit = first.q;
    let second_unit = second.q;
    return 42;
}
