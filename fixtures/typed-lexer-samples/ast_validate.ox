// CLOSED representative validator. Status 0 is local progress only, never an
// accepted AST. Status 70 means pending; unimplemented families are not a new
// language restriction. No resolver/type entry is reachable from this module.
pub struct State { pub cursor: i32, pub count: i32, pub rows: i32, pub next: i32, pub error: i32 }
pub fn clear(values: &mut [i32]) -> () {
    let mut i = 0;
    while i < 129 { values[i] = 0; i = i + 1; }
    return;
}
fn row(value: i32, rows: i32) -> bool { return value > 0 && value <= rows; }
fn token(value: i32, count: i32) -> bool { return value > 0 && value < count; }
// Role-specific raw bounds precede graph traversal and every indirect index.
fn roles(kind: i32, a: i32, b: i32, c: i32, d: i32, rows: i32, count: i32) -> bool {
    if kind == 1 { return (a == 0 || token(a, count)) && b <= rows && row(c, rows) && row(d, rows); }
    if kind == 2 { return row(a, rows) && b == 0 && c == 0 && d == 0; }
    if kind == 3 { return token(a, count) && b == 0 && c == 0 && d == 0; }
    if kind == 4 || kind == 11 || kind == 12 { return a == 0 && b == 0 && c == 0 && d == 0; }
    if kind == 5 { return token(a, count) && b <= rows && c == 0 && d == 0; }
    if kind == 6 || kind == 7 { return token(a, count) && b <= rows && row(c, rows) && d == 0; }
    if kind == 8 { return token(a, count) && token(b, count) && row(c, rows) && d == 0; }
    if kind == 9 || kind == 10 { return (row(a, rows) || (kind == 10 && a == 0)) && b == 0 && c == 0 && d == 0; }
    if kind == 13 || kind == 14 { return row(a, rows) && row(b, rows) && c <= rows && (kind == 13 || c == 0) && d == 0; }
    if kind == 15 { return token(a, count) && b <= 1 && c == 0 && d == 1; }
    if kind >= 16 && kind <= 18 { return a == 0 && b == 0 && c == 0 && d == 1; }
    if kind == 19 { return token(a, count) && b == 0 && c == 0 && d == 1; }
    if kind == 20 { return token(a, count) && b <= rows && c == 0 && d >= 1 && d <= 64; }
    if kind == 21 { return row(a, rows) && b == 0 && c == 0 && d >= 2 && d <= 64; }
    if kind == 22 || kind == 23 { return row(a, rows) && token(b, count) && c == 0 && d >= 2 && d <= 64; }
    return row(a, rows) && row(b, rows) && token(c, count) && d >= 2 && d <= 64;
}
pub fn raw(headers: &[i32], ab: &[i32], cd: &[i32], meta: &crate::ast_input::Header, count: i32) -> bool {
    let mut i = 0;
    while i < 129 {
        let h = headers[i]; let x = ab[i]; let y = cd[i];
        if i >= meta.rows {
            if h != 0 || x != 0 || y != 0 { return false; }
        } else {
            let kind = h % 64;
            if kind < 1 || kind > 36 || h / 4194304 > meta.rows || crate::parser_state::lo(h) > crate::parser_state::hi(h) || crate::parser_state::hi(h) > meta.used || x % 256 > 128 || x / 256 > 128 || y % 256 > 128 || y / 256 > 128 { return false; }
            if (kind == 3 || kind == 4 || kind == 5) && h / 4194304 != 0 { return false; }
            if !roles(kind, x % 256, x / 256, y % 256, y / 256, meta.rows, count) { return false; }
        }
        i = i + 1;
    }
    return true;
}
pub fn take(tokens: &[i32], state: &mut State, kind: i32, reference: i32) -> i32 {
    while state.cursor < state.count && tokens[state.cursor] % 64 == 1 { state.cursor = state.cursor + 1; }
    if state.error != 0 { return 0; }
    if state.cursor >= state.count { state.error = 64; return 0; }
    let result = tokens[state.cursor];
    if result % 64 != kind || (reference != 0 && reference != state.cursor + 1) { state.error = 64; return 0; }
    state.cursor = state.cursor + 1;
    return result;
}
pub fn claim(headers: &[i32], seen: &mut [i32], state: &mut State, id: i32, minimum: i32, maximum: i32, linked: bool) -> bool {
    if !row(id, state.rows) { state.error = 64; return false; }
    let h = headers[id - 1];
    if seen[id - 1] != 0 || h % 64 < minimum || h % 64 > maximum || (!linked && h / 4194304 != 0) { state.error = 64; return false; }
    // Claim before following children, even when allocation is deferred.
    seen[id - 1] = 1;
    return true;
}
pub fn allocate(state: &mut State, id: i32) -> bool {
    if id != state.next { state.error = 64; return false; }
    state.next = state.next + 1;
    return true;
}
pub fn span(state: &mut State, header: i32, first: i32, last: i32) -> () {
    if crate::parser_state::lo(header) != crate::parser_state::lo(first) || crate::parser_state::hi(header) != crate::parser_state::hi(last) { state.error = 64; }
    return;
}
fn ty(tokens: &[i32], headers: &[i32], ab: &[i32], seen: &mut [i32], state: &mut State, id: i32) -> () {
    if !claim(&*headers, &mut *seen, &mut *state, id, 3, 4, false) || !allocate(&mut *state, id) { return; }
    let h = headers[id - 1];
    let mut first = 0; let mut last = 0;
    if h % 64 == 3 { first = take(&*tokens, &mut *state, 2, ab[id - 1] % 256); last = first; }
    else { first = take(&*tokens, &mut *state, 22, 0); last = take(&*tokens, &mut *state, 23, 0); }
    span(&mut *state, h, first, last);
    return;
}
pub fn signature(tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], state: &mut State, id: i32) -> () {
    if !claim(&*headers, &mut *seen, &mut *state, id, 1, 1, true) || !allocate(&mut *state, id) { return; }
    let public = ab[id - 1] % 256;
    if public != 0 { take(&*tokens, &mut *state, 9, public); }
    take(&*tokens, &mut *state, 5, 0);
    let name = take(&*tokens, &mut *state, 2, 0);
    span(&mut *state, headers[id - 1], name, name);
    take(&*tokens, &mut *state, 22, 0);
    let mut parameter = ab[id - 1] / 256;
    while parameter != 0 && state.error == 0 {
        if !claim(&*headers, &mut *seen, &mut *state, parameter, 2, 2, true) || !allocate(&mut *state, parameter) { return; }
        let parameter_name = take(&*tokens, &mut *state, 2, 0);
        span(&mut *state, headers[parameter - 1], parameter_name, parameter_name);
        take(&*tokens, &mut *state, 26, 0);
        ty(&*tokens, &*headers, &*ab, &mut *seen, &mut *state, ab[parameter - 1] % 256);
        parameter = headers[parameter - 1] / 4194304;
        if parameter != 0 { take(&*tokens, &mut *state, 27, 0); }
    }
    take(&*tokens, &mut *state, 23, 0);
    take(&*tokens, &mut *state, 39, 0);
    ty(&*tokens, &*headers, &*ab, &mut *seen, &mut *state, cd[id - 1] % 256);
    return;
}
