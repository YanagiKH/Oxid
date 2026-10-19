use lib::total;
use lib::model::make_packet;
use lib::model::packet_value;
use lib::model::make_signal;
use lib::model::signal_score;
fn main() -> i32 {
    let p = make_packet();
    return packet_value(&p) + signal_score(make_signal()) + total();
}
fn test_packet() -> bool {
    let p = make_packet();
    return packet_value(&p) == 20;
}
fn test_signal() -> bool {
    return signal_score(make_signal()) == 3;
}
fn test_total() -> bool {
    return total() == 23;
}
