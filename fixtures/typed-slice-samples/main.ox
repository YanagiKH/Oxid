mod buffers;
mod stats;

fn main() -> i32 {
    let mut a = [1, 2];
    let mut b = [3, 4, 5];
    let mut empty: [i32; 0] = [];
    let first = crate::buffers::relay(&mut a);
    let second = crate::buffers::relay(&mut b);
    let zero = crate::buffers::relay(&mut empty);
    return first * 100 + second + zero;
}
