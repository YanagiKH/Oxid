// Acyclic expression validation. The explicit stack separates early ownership
// from canonical allocation, and only argument roots may retain next links.
struct Walk { node: i32, floor: i32, stack: i32, linked: bool }
fn precedence(kind: i32) -> i32 {
    if kind == 36 { return 1; }
    if kind == 35 { return 2; }
    if kind >= 29 { return 3; }
    if kind <= 25 { return 4; }
    return 5;
}
fn operator(kind: i32) -> i32 {
    if kind == 24 { return 41; }
    if kind == 25 { return 40; }
    if kind <= 28 { return kind + 16; }
    if kind <= 30 { return kind + 1; }
    if kind <= 34 { return kind + 4; }
    if kind == 35 { return 33; }
    return 34;
}
fn atom(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], state: &mut crate::ast_validate::State, id: i32) -> () {
    let h = headers[id - 1]; let kind = h % 64;
    let mut first = 0; let mut last = 0;
    if kind == 15 {
        if ab[id - 1] / 256 != 0 { first = crate::ast_validate::take(&*tokens, &mut *state, 40, 0); }
        last = crate::ast_validate::take(&*tokens, &mut *state, 3, ab[id - 1] % 256);
        if first == 0 { first = last; }
        if state.error != 0 { return; }
        let mut position = crate::parser_state::lo(last);
        while position < crate::parser_state::hi(last) {
            if codes[position] < 48 || codes[position] > 57 { state.error = 64; return; }
            position = position + 1;
        }
    } else {
        if kind == 18 {
            first = crate::ast_validate::take(&*tokens, &mut *state, 22, 0);
            last = crate::ast_validate::take(&*tokens, &mut *state, 23, 0);
        } else {
            let mut expected = 2;
            if kind == 16 { expected = 21; }
            if kind == 17 { expected = 20; }
            first = crate::ast_validate::take(&*tokens, &mut *state, expected, ab[id - 1] % 256);
            last = first;
        }
    }
    crate::ast_validate::span(&mut *state, h, first, last);
    crate::ast_validate::allocate(&mut *state, id);
    return;
}
fn height(cd: &[i32], heights: &mut [i32], state: &mut crate::ast_validate::State, node: i32, value: i32) -> () {
    if value > 64 || cd[node - 1] / 256 != value { state.error = 64; return; }
    heights[node - 1] = value;
    return;
}
fn enter(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], seen: &mut [i32], heights: &mut [i32], frames: &mut [i32], state: &mut crate::ast_validate::State, walk: &mut Walk) -> () {
    let node = walk.node;
    if !crate::ast_validate::claim(&*headers, &mut *seen, &mut *state, node, 15, 36, walk.linked) { return; }
    walk.linked = false;
    let kind = headers[node - 1] % 64;
    if kind < 20 {
        atom(&*codes, &*tokens, &*headers, &*ab, &mut *state, node);
        heights[node - 1] = 1; walk.node = 0;
        return;
    }
    if walk.stack >= 128 { state.error = 64; return; }
    let mut phase = 3;
    if kind >= 24 {
        let level = precedence(kind);
        if level < walk.floor { state.error = 64; return; }
        phase = 1; walk.floor = level;
        if level == 3 { walk.floor = level + 1; }
    } else {
        if !crate::ast_validate::allocate(&mut *state, node) { return; }
        let mut expected = 22; let mut reference = 0;
        if kind == 20 { expected = 2; reference = ab[node - 1] % 256; }
        if kind == 22 || kind == 23 {
            expected = 32; reference = ab[node - 1] / 256;
            if kind == 22 { expected = 40; }
        }
        let first = crate::ast_validate::take(&*tokens, &mut *state, expected, reference);
        crate::ast_validate::span(&mut *state, headers[node - 1], first, headers[node - 1]);
        walk.floor = 0;
        if kind == 22 || kind == 23 { walk.floor = 6; }
        if kind == 20 {
            crate::ast_validate::take(&*tokens, &mut *state, 22, 0);
            phase = 4; walk.linked = true;
        }
    }
    walk.node = ab[node - 1] % 256;
    if kind == 20 { walk.node = ab[node - 1] / 256; walk.linked = walk.node != 0; }
    frames[walk.stack] = node * 65536 + walk.node * 16 + phase;
    walk.stack = walk.stack + 1;
    return;
}
fn resume(tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], heights: &mut [i32], frames: &mut [i32], state: &mut crate::ast_validate::State, walk: &mut Walk) -> () {
    let frame = frames[walk.stack - 1]; let node = frame / 65536;
    let kind = headers[node - 1] % 64;
    let child = frame % 65536 / 16;
    if frame % 16 == 1 {
        crate::ast_validate::take(&*tokens, &mut *state, operator(kind), cd[node - 1] % 256);
        // Binary allocation follows left and operator, and precedes right.
        if !crate::ast_validate::allocate(&mut *state, node) { return; }
        frames[walk.stack - 1] = node * 65536 + 2;
        walk.node = ab[node - 1] / 256;
        walk.floor = precedence(kind) + 1;
        return;
    }
    let mut maximum = 0; let mut last = headers[node - 1];
    if frame % 16 == 4 {
        maximum = heights[node - 1];
        if child != 0 {
            if heights[child - 1] > maximum { maximum = heights[child - 1]; }
            let next = headers[child - 1] / 4194304;
            if next != 0 {
                crate::ast_validate::take(&*tokens, &mut *state, 27, 0);
                heights[node - 1] = maximum;
                frames[walk.stack - 1] = node * 65536 + next * 16 + 4;
                walk.node = next; walk.floor = 0; walk.linked = true;
                return;
            }
        }
        last = crate::ast_validate::take(&*tokens, &mut *state, 23, 0);
    } else {
        let left = ab[node - 1] % 256;
        maximum = heights[left - 1]; last = headers[left - 1];
        if frame % 16 == 2 {
            let right = ab[node - 1] / 256;
            if heights[right - 1] > maximum { maximum = heights[right - 1]; }
            crate::ast_validate::span(&mut *state, headers[node - 1], headers[left - 1], headers[right - 1]);
            last = headers[right - 1];
        }
        if kind == 21 { last = crate::ast_validate::take(&*tokens, &mut *state, 23, 0); }
        // Canonical -digits is one signed Number, never Negate(Number(false)).
        if kind == 22 && headers[left - 1] % 64 == 15 && ab[left - 1] / 256 == 0 { state.error = 64; return; }
    }
    height(&*cd, &mut *heights, &mut *state, node, maximum + 1);
    crate::ast_validate::span(&mut *state, headers[node - 1], headers[node - 1], last);
    walk.stack = walk.stack - 1; frames[walk.stack] = 0;
    return;
}
pub fn run(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], heights: &mut [i32], frames: &mut [i32], state: &mut crate::ast_validate::State, root: i32) -> () {
    let mut walk = Walk { node: root, floor: 0, stack: 0, linked: false };
    while state.error == 0 {
        if walk.node != 0 {
            enter(&*codes, &*tokens, &*headers, &*ab, &mut *seen, &mut *heights, &mut *frames, &mut *state, &mut walk);
        } else {
            if walk.stack == 0 { return; }
            resume(&*tokens, &*headers, &*ab, &*cd, &mut *heights, &mut *frames, &mut *state, &mut walk);
        }
    }
    return;
}
