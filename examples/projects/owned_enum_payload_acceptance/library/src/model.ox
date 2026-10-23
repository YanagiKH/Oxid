pub struct Reading {
    left: i32,
    right: i32
}
pub enum Outcome {
    Ready(Reading),
    Failed(i32)
}
pub fn measure(left: i32, right: i32) -> Outcome {
    let reading = Reading { left: left, right: right };
    return Outcome::Ready(reading);
}
pub fn fail(code: i32) -> Outcome {
    return Outcome::Failed(code);
}
pub fn total(reading: Reading) -> i32 {
    return reading.left + reading.right;
}
