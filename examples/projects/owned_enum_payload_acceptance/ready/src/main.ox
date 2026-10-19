mod flow;
use lib::model::measure;
fn main() -> i32 {
    let produced = measure(19, 23);
    return crate::flow::settle(crate::flow::forward(produced));
}
