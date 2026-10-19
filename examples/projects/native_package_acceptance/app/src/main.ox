use lib::scale;
use lib::model::make_pair;
use lib::model::pair_sum;
fn main() -> i32 {
    let p = make_pair();
    return scale(pair_sum(&p));
}
