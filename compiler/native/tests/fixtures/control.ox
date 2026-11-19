fn choose(flag: bool, a: i32, b: i32) -> i32 {
    if flag { return a; } else { return b; }
}
fn main() -> i32 {
    let mut n = 0;
    let mut total = 0;
    while n < 10 {
        n = n + 1;
        if n == 2 { continue; }
        if n > 5 { break; }
        total = total + n;
    }
    return choose(total == 13, 42, -1);
}
