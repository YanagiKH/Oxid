fn byte(x: i32) -> u8 {
    return x.to_u8_checked();
}

fn relay(a: [u8; 4]) -> [u8; 4] {
    return a;
}

fn replace_second(a: &mut [u8]) -> () {
    a[1] = byte(255);
    return;
}

fn sum(a: &[u8]) -> i32 {
    let mut i = 0;
    let mut total = 0;
    while i < a.len() {
        let b = a[i];
        total = total + b.to_i32();
        i = i + 1;
    }
    return total;
}

fn main() -> i32 {
    let input = [byte(0), byte(127), byte(128), byte(255)];
    let mut bytes = relay(input);
    replace_second(&mut bytes);
    return sum(&bytes);
}
