use crate::counts::new_totals;
use crate::counts::observe;
use crate::counts::byte_count;
use crate::counts::line_count;

fn main() -> i32 {
    let mut totals = new_totals();
    if byte_count(&totals) != 0 || line_count(&totals) != 0 { return 1; }
    let first = [97, 10, 0, 255, 10];
    observe(&mut totals, &first, 5);
    if byte_count(&totals) != 5 || line_count(&totals) != 2 { return 2; }
    let tail = [98, 99, 10];
    observe(&mut totals, &tail, 2);
    if byte_count(&totals) != 7 || line_count(&totals) != 2 { return 3; }
    observe(&mut totals, &tail, 0);
    if byte_count(&totals) != 7 || line_count(&totals) != 2 { return 4; }
    return 0;
}
