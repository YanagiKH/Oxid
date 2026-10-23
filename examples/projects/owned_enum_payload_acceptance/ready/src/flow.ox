use crate::lib::model::Outcome;
use crate::lib::model::total;
fn hold(value: Outcome) -> Outcome {
    let held = value;
    return held;
}
pub fn forward(value: Outcome) -> Outcome {
    let moved = hold(value);
    return hold(moved);
}
pub fn settle(value: Outcome) -> i32 {
    match value {
        Outcome::Ready(reading) => {
            let kept = reading;
            return total(kept);
        },
        Outcome::Failed(code) => {
            return code;
        },
    }
}
