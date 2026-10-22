use crate::codec::decode;
use crate::codec::Decoded;
use crate::codec::Content;
use crate::codec::pair_bytes;

fn score(value: Decoded) -> i32 {
    match value {
        Decoded::Item(content) => {
            match content {
                Content::Pair(pair) => {
                    let bytes = pair_bytes(&pair);
                    return bytes[0] * 256 + bytes[1];
                },
                Content::Bytes(bytes) => { return bytes[0] * 256 + bytes[1]; },
                Content::Skip => { return -1; },
            }
        },
        Decoded::Rejected(code) => { return -code; },
    }
}

fn main() -> i32 {
    if score(decode([1, 255, 0, 0])) != 255 { return 1; }
    if score(decode([1, 7, 7, 15])) != 1799 { return 2; }
    if score(decode([2, 255, 0, 1])) != 65280 { return 3; }
    if score(decode([0, 255, 1, 0])) != -1 { return 4; }
    if score(decode([9, 0, 0, 9])) != -64 { return 5; }
    if score(decode([9, 0, 0, 8])) != -65 { return 6; }
    return 0;
}
