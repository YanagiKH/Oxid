pub struct Pair { low: i32, high: i32 }

pub enum Content { Pair(Pair), Bytes([i32; 2]), Skip }
pub enum Decoded { Item(Content), Rejected(i32) }

pub fn pair_bytes(pair: &Pair) -> [i32; 2] {
    return [pair.low, pair.high];
}

pub fn decode(frame: [i32; 4]) -> Decoded {
    let tag = frame[0];
    let first = frame[1];
    let second = frame[2];
    if (tag + first + second) % 256 != frame[3] {
        return Decoded::Rejected(65);
    }
    if tag == 0 { return Decoded::Item(Content::Skip); }
    if tag == 1 {
        if first <= second {
            return Decoded::Item(Content::Pair(Pair { low: first, high: second }));
        }
        return Decoded::Item(Content::Pair(Pair { low: second, high: first }));
    }
    if tag == 2 {
        return Decoded::Item(Content::Bytes([first, second]));
    }
    return Decoded::Rejected(64);
}
