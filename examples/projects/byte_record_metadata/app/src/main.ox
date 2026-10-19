use std::io::read_stdin;
use std::io::ReadStatus;
use metadata::parse;
use metadata::Parsed;
use metadata::width;
fn main() -> i32 {
    let mut input = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let read = read_stdin(&mut input);
    match read {
        ReadStatus::Full => {
            let parsed = parse(&input);
            match parsed {
                Parsed::Ready(header) => {
                    return width(&header);
                },
                Parsed::Invalid(code) => {
                    return code;
                },
            }
        },
        ReadStatus::Eof(count) => {
            return -1;
        },
        ReadStatus::IoError => {
            return -6;
        },
    }
}
