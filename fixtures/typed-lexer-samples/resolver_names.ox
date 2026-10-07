// Resolution-only helpers. Diagnostic handoff reuses State after traversal ends:
// stack = kind, locals = primary packed span, value = secondary packed span.
pub fn fail(state: &mut crate::static_state::State, kind: i32, primary: i32, secondary: i32) -> () {
    state.stack = kind; state.locals = primary; state.value = secondary;
    return;
}
pub fn name(tokens: &[i32], headers: &[i32], ab: &[i32], node: i32) -> i32 {
    if headers[node - 1] % 64 <= 2 { return headers[node - 1]; }
    return tokens[ab[node - 1] % 256 - 1];
}
pub fn local(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], locals: &[i32], target: i32, count: i32, used: i32) -> i32 {
    let mut i = 0;
    while i < count {
        let node = locals[i];
        if headers[node - 1] % 64 != 5 && crate::static_common::same_name(&*codes, name(&*tokens, &*headers, &*ab, node), target, used) { return node; }
        i = i + 1;
    }
    return 0;
}
pub fn bind(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], locals: &mut [i32], state: &mut crate::static_state::State, node: i32, rows: i32, used: i32) -> i32 {
    let target = name(&*tokens, &*headers, &*ab, node);
    let mut previous = local(&*codes, &*tokens, &*headers, &*ab, &*locals, target, state.locals, used);
    if previous == 0 { previous = crate::static_common::find_function(&*codes, &*headers, target, rows, used); }
    if previous != 0 {
        fail(&mut *state, 3, target, name(&*tokens, &*headers, &*ab, previous));
        return 2;
    }
    if !crate::static_state::local_push(&mut *locals, &mut *state, node) { return 0; }
    return 1;
}
fn primitive(codes: &[i32], header: i32) -> i32 {
    if header % 64 == 4 { return 3; }
    let start = crate::parser_state::lo(header);
    let length = crate::parser_state::hi(header) - start;
    if length == 3 && codes[start] == 105 && codes[start + 1] == 51 && codes[start + 2] == 50 { return 2; }
    if length == 4 && codes[start] == 98 && codes[start + 1] == 111 && codes[start + 2] == 111 && codes[start + 3] == 108 { return 1; }
    return 0;
}
fn unknown(headers: &[i32], resolved: &[i32], state: &mut crate::static_state::State, node: i32) -> bool {
    if node == 0 || resolved[node - 1] != 0 { return false; }
    fail(&mut *state, 4, headers[node - 1], 0);
    return true;
}
pub fn preflight(codes: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], resolved: &mut [i32], state: &mut crate::static_state::State, rows: i32, used: i32) -> () {
    let mut i = 0;
    let mut def = 0;
    // Global conflicts precede unknown-type preflight and every body error.
    while i < rows {
        let kind = headers[i] % 64;
        if kind == 1 {
            let previous = crate::static_common::find_function(&*codes, &*headers, headers[i], i, used);
            if previous != 0 { fail(&mut *state, 3, headers[i], headers[previous - 1]); return; }
            def = def + 1; resolved[i] = def;
        }
        if kind == 3 || kind == 4 { resolved[i] = primitive(&*codes, headers[i]); }
        i = i + 1;
    }
    i = 0;
    while i < rows {
        if headers[i] % 64 == 1 {
            let mut parameter = ab[i] / 256;
            while parameter != 0 {
                if unknown(&*headers, &*resolved, &mut *state, ab[parameter - 1] % 256) { return; }
                parameter = headers[parameter - 1] / 4194304;
            }
            if unknown(&*headers, &*resolved, &mut *state, cd[i] % 256) { return; }
        }
        // Blocks are allocated in canonical preorder. Inspect each complete
        // block's own statement list before later nested block rows.
        if headers[i] % 64 == 5 {
            let mut statement = ab[i] / 256;
            while statement != 0 {
                let kind = headers[statement - 1] % 64;
                if (kind == 6 || kind == 7) && unknown(&*headers, &*resolved, &mut *state, ab[statement - 1] / 256) { return; }
                statement = headers[statement - 1] / 4194304;
            }
        }
        i = i + 1;
    }
    return;
}
