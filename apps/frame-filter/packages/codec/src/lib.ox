pub struct Pair { low: u8, high: u8 }

pub enum Content { Pair(Pair), Bytes([i32; 2]), Skip }
pub enum Decoded { Item(Content), Rejected(i32) }

pub fn pair_bytes(pair: &Pair) -> [i32; 2] {
    let low = pair.low;
    let high = pair.high;
    return [low.to_i32(), high.to_i32()];
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
        let first_byte = first.to_u8_checked();
        let second_byte = second.to_u8_checked();
        if first <= second {
            return Decoded::Item(Content::Pair(Pair { low: first_byte, high: second_byte }));
        }
        return Decoded::Item(Content::Pair(Pair { low: second_byte, high: first_byte }));
    }
    if tag == 2 {
        return Decoded::Item(Content::Bytes([first, second]));
    }
    return Decoded::Rejected(64);
}
