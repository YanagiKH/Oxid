use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_extend;

// Single-file typed source compile client. With base() == 7:
// scalar 21 + record 9 + enum 2 + owned buffer 6 + bytes 4 == 42.
struct Pair {
    left: i32,
    right: i32
}
enum Outcome {
    Ready(Pair),
    Failed(i32)
}
fn base() -> i32 {
    return 7;
}
fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}
fn measure(left: i32, right: i32) -> Outcome {
    if left < 0 {
        return Outcome::Failed(left);
    }
    return Outcome::Ready(Pair { left: left, right: right });
}
fn score(outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Ready(pair) => {
            return pair.left + pair.right;
        },
        Outcome::Failed(code) => {
            return code;
        },
    }
}
fn total(bytes: &[u8]) -> i32 {
    let mut sum = 0;
    let mut offset = 0;
    while offset < bytes.len() {
        let current = bytes[offset];
        sum = sum + current.to_i32();
        offset = offset + 1;
    }
    return sum;
}
fn buffer_part() -> i32 {
    let mut owner = new();
    let bytes = [byte(1), byte(2), byte(3)];
    let grown = try_extend(&mut owner, &bytes);
    match grown {
        GrowStatus::Ok => {
            return total(&owner);
        },
        GrowStatus::CapacityOverflow => {
            return -100;
        },
        GrowStatus::AllocationFailed => {
            return -101;
        },
        GrowStatus::MemoryLimit => {
            return -102;
        },
    }
}
fn main() -> i32 {
    let pair = Pair { left: base(), right: 2 };
    let scalar = base() * 3;
    let record = pair.left + pair.right;
    let outcome = score(measure(base(), 1)) + score(measure(-6, 9));
    let text = [byte(79), byte(120), byte(194), byte(162)];
    return scalar + record + outcome + buffer_part() + text.len();
}
