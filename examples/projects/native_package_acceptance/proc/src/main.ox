use lib::scale;
use std::io::write_stdout;
fn main() -> i32 {
    let bytes = [80, 75, 71, 10];
    let written = write_stdout(&bytes);
    return scale(1);
}
