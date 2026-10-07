// Acyclic explicit traversal; semantic block cells own accumulated F/R/B/C bits.
// State.locals is the current result type, State.value the last completed row.
fn body(tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], resolved: &[i32], semantic: &mut [i32], frames: &mut [i32], state: &mut crate::static_state::State) -> bool {
    while state.stack > 0 {
        let frame = frames[state.stack - 1];
        let node = frame / 16;
        let phase = frame % 16;
        let kind = headers[node - 1] % 64;
        let a = ab[node - 1] % 256;
        let b = ab[node - 1] / 256;
        let mut child = 0;
        let mut next_phase = phase + 1;
        if kind == 5 || kind == 20 {
            next_phase = 1;
            if phase == 0 {
                if kind == 5 { semantic[node - 1] = 1; }
                child = b;
            } else {
                if kind == 5 {
                    semantic[node - 1] = crate::typed_statement::union(semantic[node - 1] - 1, crate::typed_statement::flow(&*headers, &*ab, &*cd, &*semantic, state.value));
                }
                child = headers[state.value - 1] / 4194304;
            }
            if kind == 5 && child != 0 && semantic[node - 1] % 2 == 0 {
                let mut error = 13;
                if semantic[node - 1] == 2 { error = 12; }
                crate::typed_diagnostic::fail(&mut *state, error, headers[child - 1], 0, 0, 0, 0);
                return state.stack >= 0;
            }
        } else { if kind >= 6 && kind <= 8 {
            if phase == 0 { child = cd[node - 1] % 256; }
        } else { if kind == 13 || kind == 14 {
            if phase == 0 { child = a; }
            if phase == 1 {
                if !crate::typed_diagnostic::expect(&mut *state, 1, semantic[a - 1], headers[a - 1], 0, 0) { return state.stack >= 0; }
                child = b;
            }
            if phase == 2 && kind == 13 { child = cd[node - 1] % 256; }
        } else {
            if kind == 9 || kind == 10 || kind >= 21 {
                if phase == 0 { child = a; }
                if phase == 1 && kind >= 24 { child = b; }
            }
        } } }
        if child != 0 {
            if !crate::static_state::resume(&mut *frames, &*state, next_phase) || !crate::static_state::push(&mut *frames, &mut *state, child, 0) { return false; }
        } else {
            let mut completed = true;
            if kind == 20 { completed = crate::typed_expression::call(&*headers, &*ab, &*resolved, &mut *semantic, &mut *state, node); }
            else { if kind >= 15 { completed = crate::typed_expression::finish(&*headers, &*ab, &*resolved, &mut *semantic, &mut *state, node); }
            else { if kind >= 6 && kind <= 10 { completed = crate::typed_statement::finish(&*tokens, &*headers, &*ab, &*cd, &*resolved, &mut *semantic, &mut *state, node); } } }
            if !completed { return state.stack >= 0; }
            state.value = crate::static_state::pop(&mut *frames, &mut *state);
            if state.value == 0 { return false; }
        }
    }
    state.locals = 0; state.value = 0;
    return true;
}
pub fn run(tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], resolved: &[i32], semantic: &mut [i32], frames: &mut [i32], state: &mut crate::static_state::State, rows: i32) -> bool {
    if state.stack != 0 || state.locals != 0 || state.value != 0 { return false; }
    let mut i = 0;
    while i < 129 {
        if frames[i] != 0 || semantic[i] != 0 { return false; }
        if i < rows {
            let kind = headers[i] % 64;
            if kind == 1 { semantic[i] = resolved[cd[i] % 256 - 1]; }
            if kind == 2 { semantic[i] = resolved[ab[i] % 256 - 1]; }
        }
        i = i + 1;
    }
    i = 0;
    while i < rows {
        if headers[i] % 64 == 1 {
            let block = cd[i] / 256;
            state.locals = semantic[i];
            if !crate::static_state::push(&mut *frames, &mut *state, block, 0) { return false; }
            let complete = body(&*tokens, &*headers, &*ab, &*cd, &*resolved, &mut *semantic, &mut *frames, &mut *state);
            let mut clear = 0;
            while clear < 129 { frames[clear] = 0; clear = clear + 1; }
            if !complete { return false; }
            if state.stack != 0 { return true; }
            if semantic[block - 1] != 2 {
                let end = crate::parser_state::hi(headers[block - 1]);
                crate::typed_diagnostic::fail(&mut *state, 11, 64 * (end - 1 + 256 * end), 0, 0, 0, 0);
                return state.stack >= 0;
            }
        }
        i = i + 1;
    }
    return true;
}
