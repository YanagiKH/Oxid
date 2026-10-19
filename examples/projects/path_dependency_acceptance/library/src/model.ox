use shared::base;
pub struct Packet {
    n: i32
}
pub enum Signal {
    Ready(i32),
    Idle
}
fn hidden() -> i32 {
    return 1;
}
pub fn make_packet() -> Packet {
    return Packet { n: base() };
}
pub fn packet_value(p: &Packet) -> i32 {
    return p.n;
}
pub fn make_signal() -> Signal {
    return Signal::Ready(3);
}
pub fn signal_score(s: Signal) -> i32 {
    match s {
        Signal::Ready(n) => {
            return n;
        },
        Signal::Idle => {
            return 0;
        },
    }
}
