pub fn relay(a: [i32; 8]) -> [i32; 8] { return a; }
fn filter(a: &mut [i32; 8], s: &mut crate::stats::Stats) -> () {
    let mut read = 0;
    let mut write = 0;
    while read < a.len() {
        let value = a[read];
        read = read + 1;
        if value < 0 { continue; }
        a[write] = value;
        write = write + 1;
        crate::stats::record(&mut *s, value);
    }
    while write < a.len() {
        a[write] = 0;
        write = write + 1;
    }
    return;
}
pub fn dispatch(a: &mut [i32; 8], s: &mut crate::stats::Stats) -> () {
    filter(&mut *a, &mut *s);
    return;
}
pub fn checksum(a: &[i32; 8]) -> i32 {
    let mut i = 0;
    let mut total = 0;
    while i < a.len() {
        total = total + (i + 1) * a[i];
        i = i + 1;
    }
    return total;
}
