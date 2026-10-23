pub struct Totals { bytes: i32, lines: i32 }

pub fn new_totals() -> Totals {
    return Totals { bytes: 0, lines: 0 };
}

pub fn observe(totals: &mut Totals, bytes: &[i32], count: i32) -> () {
    let mut index = 0;
    while index < count {
        totals.bytes = totals.bytes + 1;
        if bytes[index] == 10 {
            totals.lines = totals.lines + 1;
        }
        index = index + 1;
    }
    return;
}

pub fn byte_count(totals: &Totals) -> i32 {
    return totals.bytes;
}

pub fn line_count(totals: &Totals) -> i32 {
    return totals.lines;
}
