use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_extend;
pub struct Pair {
    left: i32,
    right: i32
}
pub enum Outcome {
    Ready(Pair),
    Failed(i32)
}
pub fn make_pair(left: i32, right: i32) -> Pair {
    return Pair { left: left, right: right };
}
pub fn pair_sum(pair: &Pair) -> i32 {
    return pair.left + pair.right;
}
pub fn measure(left: i32, right: i32) -> Outcome {
    if left < 0 {
        return Outcome::Failed(left);
    }
    return Outcome::Ready(make_pair(left, right));
}
pub fn score(outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Ready(pair) => {
            return pair_sum(&pair);
        },
        Outcome::Failed(code) => {
            return code;
        },
    }
}
fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}
pub fn filled() -> ByteBuf {
    let mut owner = new();
    let bytes = [byte(1), byte(2), byte(3)];
    let grown = try_extend(&mut owner, &bytes);
    match grown {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => {},
        GrowStatus::AllocationFailed => {},
        GrowStatus::MemoryLimit => {},
    }
    return owner;
}
pub fn byte_total(bytes: &[u8]) -> i32 {
    let mut total = 0;
    let mut offset = 0;
    while offset < bytes.len() {
        let current = bytes[offset];
        total = total + current.to_i32();
        offset = offset + 1;
    }
    return total;
}
