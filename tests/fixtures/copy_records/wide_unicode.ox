#[derive(Copy)]
struct Pair { bits: u128, letter: char }
#[derive(Copy)]
struct Holder<T: Copy> { value: T }
fn identity<T: Copy>(value: T) -> T { return value; }
fn main() -> i32 {
    let pair = Pair { bits: 340282366920938463463374607431768211455u128, letter: '雪' };
    let holder = Holder::<Pair> { value: pair };
    let second = identity(holder);
    let Holder { value: copy } = second;
    let Holder { value: original } = holder;
    if pair.bits == copy.bits && original.letter == pair.letter && copy.letter == '雪' {
        return 42;
    }
    return 1;
}
