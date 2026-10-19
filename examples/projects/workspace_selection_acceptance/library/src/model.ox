pub struct Pair {
    a: i32,
    b: i32
}
pub fn make_pair() -> Pair {
    return Pair { a: 4, b: 7 };
}
pub fn pair_sum(p: &Pair) -> i32 {
    return p.a + p.b;
}
