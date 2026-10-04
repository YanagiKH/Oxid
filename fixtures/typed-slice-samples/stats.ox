pub fn sum(p: &[i32]) -> i32 {
    let mut i = 0;
    let mut total = 0;
    while i < p.len() {
        total = total + p[i];
        i = i + 1;
    }
    return total;
}
