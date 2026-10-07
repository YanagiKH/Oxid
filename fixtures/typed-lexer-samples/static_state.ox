// Phase-local scalar control; parser continuations are released before use.
pub struct State { pub stack: i32, pub locals: i32, pub value: i32 }
pub fn release(frames: &mut [i32], parser: &crate::parser_state::State) -> bool {
    if parser.error != 0 || parser.mode != 9 || parser.stack != 1 || frames.len() != 129 || frames[0] != 1 { return false; }
    let mut i = 1;
    while i < 129 { if frames[i] != 0 { return false; } i = i + 1; }
    frames[0] = 0;
    return true;
}
pub fn push(frames: &mut [i32], state: &mut State, node: i32, mask: i32) -> bool {
    if state.stack < 0 || state.stack >= 128 || node < 1 || node > 128 || mask < 0 || mask > 15 { return false; }
    frames[state.stack] = node * 16 + mask;
    state.stack = state.stack + 1;
    return true;
}
pub fn resume(frames: &mut [i32], state: &State, mask: i32) -> bool {
    if state.stack < 1 || state.stack > 128 || mask < 0 || mask > 15 { return false; }
    let node = frames[state.stack - 1] / 16;
    if node < 1 || node > 128 { return false; }
    frames[state.stack - 1] = node * 16 + mask;
    return true;
}
pub fn pop(frames: &mut [i32], state: &mut State) -> i32 {
    if state.stack < 1 || state.stack > 128 { return 0; }
    state.stack = state.stack - 1;
    let node = frames[state.stack] / 16;
    frames[state.stack] = 0;
    return node;
}
pub fn local_push(locals: &mut [i32], state: &mut State, node: i32) -> bool {
    if state.locals < 0 || state.locals >= 128 || node < 1 || node > 128 { return false; }
    locals[state.locals] = node;
    state.locals = state.locals + 1;
    return true;
}
pub fn local_pop(locals: &mut [i32], state: &mut State) -> i32 {
    if state.locals < 1 || state.locals > 128 { return 0; }
    state.locals = state.locals - 1;
    let node = locals[state.locals];
    locals[state.locals] = 0;
    return node;
}
