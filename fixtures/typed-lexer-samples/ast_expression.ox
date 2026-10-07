// Representative atoms and binary expressions. Group/unary/call validation is
// explicitly pending. The iterative walk claims each row before following its
// left child, independently of the later binary allocation event.
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
pub fn run(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], heights: &mut [i32], frames: &mut [i32], state: &mut crate::ast_validate::State, root: i32) -> () {
    let mut current = root; let mut floor = 0; let mut stack = 0;
    while state.error == 0 {
        if current != 0 {
            if !crate::ast_validate::claim(&*headers, &mut *seen, &mut *state, current, 15, 36, false) { return; }
            let kind = headers[current - 1] % 64;
            if kind >= 24 {
                let level = precedence(kind);
                if level < floor || stack >= 128 { state.error = 64; return; }
                frames[stack] = current * 16 + 1; stack = stack + 1;
                current = ab[current - 1] % 256;
                floor = level;
                if level == 3 { floor = floor + 1; }
            } else {
                if kind >= 20 { state.error = 70; return; }
                atom(&*codes, &*tokens, &*headers, &*ab, &mut *state, current);
                heights[current - 1] = 1;
                current = 0;
            }
        } else {
            if stack == 0 { return; }
            let frame = frames[stack - 1]; let node = frame / 16;
            let kind = headers[node - 1] % 64;
            if frame % 16 == 1 {
                crate::ast_validate::take(&*tokens, &mut *state, operator(kind), cd[node - 1] % 256);
                // Actual parser order: complete left, operator, binary row,
                // then right. Ownership was already claimed on entry.
                if !crate::ast_validate::allocate(&mut *state, node) { return; }
                frames[stack - 1] = node * 16 + 2;
                current = ab[node - 1] / 256;
                floor = precedence(kind) + 1;
            } else {
                let left = ab[node - 1] % 256; let right = ab[node - 1] / 256;
                let mut height = heights[left - 1];
                if heights[right - 1] > height { height = heights[right - 1]; }
                height = height + 1;
                if height > 64 || cd[node - 1] / 256 != height { state.error = 64; return; }
                heights[node - 1] = height;
                crate::ast_validate::span(&mut *state, headers[node - 1], headers[left - 1], headers[right - 1]);
                stack = stack - 1; frames[stack] = 0;
            }
        }
    }
    return;
}
