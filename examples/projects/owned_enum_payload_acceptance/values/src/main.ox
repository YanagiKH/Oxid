struct Packet {
    left: i32,
    right: i32
}
struct Nothing {
}
enum Inner {
    Hit(Packet),
    Miss
}
enum Slot {
    Pair([i32; 2]),
    Nested(Inner),
    Blank(Nothing),
    Hollow([i32; 0]),
    Unit(()),
    Failed(i32)
}
fn pair() -> [i32; 2] {
    return [10, 4];
}
fn hit() -> Inner {
    return Inner::Hit(Packet { left: 3, right: 2 });
}
fn nothing() -> Nothing {
    return Nothing {};
}
fn hollow() -> [i32; 0] {
    let items: [i32; 0] = [];
    return items;
}
fn relay(slot: Slot) -> Slot {
    let held = slot;
    return held;
}
fn score(slot: Slot) -> i32 {
    match slot {
        Slot::Pair(items) => {
            let moved = items;
            return moved[0] + moved[1];
        },
        Slot::Nested(inner) => {
            match inner {
                Inner::Hit(packet) => {
                    return packet.left * packet.right;
                },
                Inner::Miss => {
                    return 100;
                },
            }
        },
        Slot::Blank(empty) => {
            empty;
            return 1;
        },
        Slot::Hollow(items) => {
            items;
            return 2;
        },
        Slot::Unit(unit) => {
            unit;
            return 3;
        },
        Slot::Failed(code) => {
            return code;
        },
    }
}
fn main() -> i32 {
    let direct_pair = score(relay(Slot::Pair(pair())));
    let grouped_pair = score(relay(Slot::Pair((pair()))));
    let direct_nested = score(relay(Slot::Nested(hit())));
    let grouped_nested = score(relay(Slot::Nested(((hit())))));
    let direct_blank = score(relay(Slot::Blank(nothing())));
    let grouped_hollow = score(relay(Slot::Hollow((hollow()))));
    let unit_value = score(relay(Slot::Unit(())));
    let mut current = Slot::Failed(-7);
    current = Slot::Pair(pair());
    current = Slot::Nested((hit()));
    current = Slot::Failed(-4);
    let replaced = score(current);
    let discarded = Slot::Pair((pair()));
    discarded;
    return direct_pair + grouped_pair + direct_nested + grouped_nested + direct_blank + grouped_hollow + unit_value + replaced;
}
