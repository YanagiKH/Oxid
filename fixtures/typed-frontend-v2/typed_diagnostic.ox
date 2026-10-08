// Terminal State layout: kind:4, expected:8, actual:8, label:3.
// Traversal ownership ends before this checked packing replaces stack/locals/value.
// A negative stack denotes an internal failure, never an emitted diagnostic.
pub fn fail(state: &mut crate::static_state::State, kind: i32, primary: i32, secondary: i32, label: i32, expected: i32, actual: i32) -> bool {
    state.stack = -1;
    let start = crate::parser_state::lo(primary);
    let end = crate::parser_state::hi(primary);
    let first = crate::parser_state::lo(secondary);
    let last = crate::parser_state::hi(secondary);
    if kind < 1 || kind > 14 || expected < 0 || expected > 128 || actual < 0 || actual > 128 || label < 0 || label > 4 || start >= end || end > 255 { return false; }
    if label == 0 { if secondary != 0 { return false; } }
    else { if first >= last || last > 255 { return false; } }
    if kind == 8 {
        if expected < 1 || expected > 3 || actual < 1 || actual > 3 || expected == actual { return false; }
    } else { if kind == 10 {
        if expected == actual { return false; }
    } else { if expected != 0 || actual != 0 { return false; } } }
    state.stack = kind + 16 * expected + 4096 * actual + 1048576 * label;
    state.locals = 64 * (start + 256 * end);
    state.value = 64 * (first + 256 * last);
    return false;
}
pub fn expect(state: &mut crate::static_state::State, expected: i32, actual: i32, primary: i32, secondary: i32, label: i32) -> bool {
    if expected == actual { return true; }
    return fail(&mut *state, 8, primary, secondary, label, expected, actual);
}
