use std::io::read_stdin as input;
use std::io::ReadStatus as Status;
fn receive(bytes: &mut [i32]) -> Status { return input(&mut *bytes); }
fn main() -> i32 {
    let mut bytes = [-7, -7, -7];
    let status = receive(&mut bytes);
    let sum = bytes[0] + bytes[1] + bytes[2];
    match status {
        Status::Eof(n) => { return n * 1000 + sum; },
        Status::Full => { return 10000 + sum; },
        Status::IoError => { return -10000 + sum; },
    }
}
