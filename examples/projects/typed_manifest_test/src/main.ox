mod answer;
fn main() -> i32 {
    return crate::answer::value();
}
fn test_zulu() -> bool {
    return crate::answer::value() == 7;
}
fn test_alpha() -> () {
    return;
}
fn test_zero() -> i32 {
    return crate::answer::value() - 7;
}
