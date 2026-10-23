use crate::codec::decode;
use crate::codec::Decoded;
use crate::output::emit;
use std::io::read_stdin;
use std::io::ReadStatus;

fn main() -> i32 {
    while true {
        let mut frame = [0, 0, 0, 0];
        let status = read_stdin(&mut frame);
        match status {
            ReadStatus::Full => {
                let decoded = decode(frame);
                match decoded {
                    Decoded::Item(content) => {
                        let result = emit(content);
                        if result != 0 { return result; }
                    },
                    Decoded::Rejected(code) => { return code; },
                }
            },
            ReadStatus::Eof(count) => {
                if count == 0 { return 0; }
                return 66;
            },
            ReadStatus::IoError => { return 74; },
        }
    }
    return 74;
}
