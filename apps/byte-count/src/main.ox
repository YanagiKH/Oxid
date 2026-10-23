use crate::counts::new_totals;
use crate::counts::observe;
use crate::report::write_report;
use std::io::read_stdin;
use std::io::ReadStatus;

fn main() -> i32 {
    let mut totals = new_totals();
    let mut bytes = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    while true {
        let status = read_stdin(&mut bytes);
        match status {
            ReadStatus::Full => {
                observe(&mut totals, &bytes, 32);
            },
            ReadStatus::Eof(count) => {
                observe(&mut totals, &bytes, count);
                return write_report(&totals);
            },
            ReadStatus::IoError => {
                return 2;
            },
        }
    }
    return 2;
}
