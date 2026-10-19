use lib::scale;
use lib::model::make_pair;
use lib::model::pair_sum;
fn main() -> i32 {
    let p = make_pair();
    return scale(pair_sum(&p));
}
fn test_scale() -> bool {
    return scale(2) == 6;
}
fn test_pair() -> bool {
    let p = make_pair();
    return pair_sum(&p) == 11;
}
