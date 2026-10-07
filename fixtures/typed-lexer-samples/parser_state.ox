// Early parser control/storage. Complete grammar remains an explicit later gate.
pub struct State {
    pub cursor: i32, pub limit: i32, pub used: i32,
    pub rows: i32, pub stack: i32, pub items: i32, pub tail: i32,
    pub mode: i32, pub value: i32, pub depth: i32, pub context: i32, pub floor: i32,
    pub error: i32, pub detail: i32, pub start: i32, pub end: i32,
}
pub fn lo(token: i32) -> i32 { return token / 64 % 256; }
pub fn hi(token: i32) -> i32 { return token / 16384 % 256; }
pub fn bump(tokens: &[i32], state: &mut State) -> i32 {
    let id = state.cursor + 1;
    if tokens[state.cursor] % 64 != 47 {
        state.cursor = state.cursor + 1;
        while state.cursor < state.limit && tokens[state.cursor] % 64 == 1 {
            state.cursor = state.cursor + 1;
        }
    }
    return id;
}
pub fn fail(codes: &[i32], tokens: &[i32], state: &mut State, message: i32) -> () {
    let token = tokens[state.cursor];
    let kind = token % 64;
    let start = lo(token);
    let end = hi(token);
    let unsupported = kind == 45 || kind == 7 || kind == 8 || kind == 9 || kind == 3 || kind == 40 || kind == 41 || kind == 4 || kind == 6 || kind == 10 || kind == 11 || (kind == 2 && end - start == 2 && codes[start] == 97 && codes[start + 1] == 115);
    state.error = 1;
    state.detail = message;
    if unsupported { state.error = 2; state.detail = 1; }
    state.start = start;
    state.end = end;
    return;
}
pub fn expect(codes: &[i32], tokens: &[i32], state: &mut State, kind: i32, message: i32) -> i32 {
    if tokens[state.cursor] % 64 != kind {
        fail(&*codes, &*tokens, &mut *state, message);
        return 0;
    }
    return bump(&*tokens, &mut *state);
}
pub fn row(headers: &mut [i32], state: &mut State, kind: i32, start: i32, end: i32) -> i32 {
    if state.rows >= 128 { state.error = 9; return 0; }
    let at = state.rows;
    state.rows = at + 1;
    headers[at] = kind + 64 * (start + 256 * end);
    return at + 1;
}
pub fn span(headers: &mut [i32], id: i32, start: i32, end: i32) -> () {
    let old = headers[id - 1];
    headers[id - 1] = old % 64 + 64 * (start + 256 * end) + 4194304 * (old / 4194304);
    return;
}
pub fn link(headers: &mut [i32], id: i32, next: i32) -> () {
    headers[id - 1] = headers[id - 1] % 4194304 + 4194304 * next;
    return;
}
pub fn low(column: &mut [i32], id: i32, value: i32) -> () {
    column[id - 1] = value + 256 * (column[id - 1] / 256);
    return;
}
pub fn high(column: &mut [i32], id: i32, value: i32) -> () {
    column[id - 1] = column[id - 1] % 256 + 256 * value;
    return;
}
pub fn push(frames: &mut [i32], state: &mut State, phase: i32, node: i32, aux: i32) -> () {
    if state.stack >= 129 { state.error = 9; return; }
    frames[state.stack] = node * 65536 + phase + 32 * aux;
    state.stack = state.stack + 1;
    return;
}
pub fn pop(frames: &mut [i32], state: &mut State) -> i32 {
    if state.stack <= 1 { state.error = 9; return 0; }
    state.stack = state.stack - 1;
    let frame = frames[state.stack];
    frames[state.stack] = 0;
    return frame;
}
