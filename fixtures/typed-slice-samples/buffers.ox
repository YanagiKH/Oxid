fn bump(p: &mut [i32]) -> () {
    let mut i = 0;
    while i < p.len() {
        p[i] = p[i] + 1;
        i = i + 1;
    }
    return;
}

pub fn relay(p: &mut [i32]) -> i32 {
    bump(&mut *p);
    return crate::stats::sum(&*p);
}
