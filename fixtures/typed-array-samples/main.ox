mod samples;
mod stats;
fn main() -> i32 {
    let original = [5, -2, 7, 0, -1, 9, 4, -3];
    let mut values = crate::samples::relay(original);
    let mut result = crate::stats::create();
    crate::samples::dispatch(&mut values, &mut result);
    return crate::stats::count(&result) * 1000
        + crate::stats::sum(&result) * 10
        + crate::samples::checksum(&values);
}
