use crate::codec::Content;
use crate::codec::pair_bytes;
use std::io::write_stdout;
use std::io::WriteStatus;

pub fn emit(content: Content) -> i32 {
    let mut tag = 0;
    let mut payload = [0, 0];
    match content {
        Content::Pair(pair) => {
            tag = 1;
            payload = pair_bytes(&pair);
        },
        Content::Bytes(bytes) => {
            tag = 2;
            payload = bytes;
        },
        Content::Skip => { return 0; },
    }
    let first = payload[0];
    let second = payload[1];
    let frame = [tag, first, second, (tag + first + second) % 256];
    let status = write_stdout(&frame);
    match status {
        WriteStatus::Complete => { return 0; },
        WriteStatus::InvalidInput => { return 75; },
        WriteStatus::IoError(count) => { return 75; },
    }
}
