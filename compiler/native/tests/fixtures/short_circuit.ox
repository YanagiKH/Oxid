fn main() -> bool {
    let a = false && (1 / 0 == 0);
    let b = true || (2147483647 + 1 == 0);
    return (!a && b) || false;
}
