/// Type and value aliases sharing one spelling.
pub mod alias_pairs;
use units::origin as Local;

/// A local record whose name is also a function alias.
pub struct Local {
    pub seed: i32,
}

/// Returns the local `Local` record, not `units::origin`.
pub fn parse_value(seed: i32) -> Local {
    let start = Local();
    return Local { seed: seed + start.x };
}

/// An `i32`-only signature lists no import.
pub fn plain_count(n: i32) -> i32 {
    return n;
}
