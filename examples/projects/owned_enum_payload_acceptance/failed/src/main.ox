mod flow;
use lib::model::fail;
fn main() -> i32 {
    let produced = fail(-7);
    return crate::flow::settle(crate::flow::forward(produced));
}
