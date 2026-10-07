// Expression continuations share the charged row/frame carrier. No empty
// precedence tier pushes a frame; the driver alone schedules the next step.
fn precedence(kind: i32) -> i32 {
    if kind == 34 { return 1; }
    if kind == 33 { return 2; }
    if kind == 30 || kind == 31 || (kind >= 35 && kind <= 38) { return 3; }
    if kind == 40 || kind == 41 { return 4; }
    if kind >= 42 && kind <= 44 { return 5; }
    return 0;
}

fn row_kind(kind: i32) -> i32 {
    if kind == 41 { return 24; }
    if kind == 40 { return 25; }
    if kind >= 42 && kind <= 44 { return kind - 16; }
    if kind == 30 || kind == 31 { return kind - 1; }
    if kind >= 35 && kind <= 38 { return kind - 4; }
    if kind == 33 { return 35; }
    return 36;
}

fn extend(tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, level: i32) -> () {
    let token = tokens[state.cursor];
    let operator = crate::parser_state::bump(&*tokens, &mut *state);
    let tag = row_kind(token % 64);
    let origin = crate::parser_state::lo(headers[state.value - 1]);
    let node = crate::parser_state::row(&mut *headers, &mut *state, tag, origin, crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *ab, node, state.value);
    crate::parser_state::low(&mut *cd, node, operator);
    let aux = crate::parser_state::expression_aux(&*state);
    let mut phase = 16;
    if level == 3 { phase = 17; }
    crate::parser_state::push(&mut *frames, &mut *state, phase, node, aux);
    state.floor = level + 1;
    state.mode = 1;
    return;
}

fn reduce(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    let phase = frame % 32;
    let node = frame / 65536;
    let value = state.value;
    let start = crate::parser_state::lo(headers[node - 1]);
    let mut end = crate::parser_state::hi(headers[value - 1]);
    let mut height = cd[value - 1] / 256 + 1;
    if phase == 14 {
        let close = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 23, 22);
        if state.error != 0 { return; }
        end = crate::parser_state::hi(tokens[close - 1]);
        crate::parser_atom::postfix(&*codes, &*tokens, &mut *state);
        if state.error != 0 { return; }
    }
    if phase == 17 && precedence(tokens[state.cursor] % 64) == 3 {
        crate::parser_state::fail(&*codes, &*tokens, &mut *state, 23);
        return;
    }
    if phase == 16 || phase == 17 {
        let left = ab[node - 1] % 256;
        let left_height = cd[left - 1] / 256 + 1;
        if left_height > height { height = left_height; }
        crate::parser_state::high(&mut *ab, node, value);
    } else {
        crate::parser_state::low(&mut *ab, node, value);
    }
    crate::parser_state::span(&mut *headers, node, start, end);
    if height > 64 {
        state.error = 3; state.detail = 1;
        state.start = start; state.end = end;
        return;
    }
    crate::parser_state::high(&mut *cd, node, height);
    let finished = crate::parser_state::pop(&mut *frames, &mut *state);
    crate::parser_state::restore_expression(&mut *state, finished);
    state.value = node;
    return;
}

// True means that the entire expression returned to its grammar caller. A
// single transition otherwise extends or finishes one expression owner.
pub fn resume(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> bool {
    let phase = frames[state.stack - 1] % 32;
    if phase == 19 {
        crate::parser_call::resume(&*codes, &*tokens, &mut *headers, &mut *cd, &mut *frames, &mut *state);
        return false;
    }
    let level = precedence(tokens[state.cursor] % 64);
    if level != 0 && level >= state.floor {
        extend(&*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state, level);
        return false;
    }
    if phase == 15 {
        crate::parser_call::argument_done(&mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
        return false;
    }
    if phase == 13 || phase == 14 || phase == 16 || phase == 17 {
        reduce(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
        return false;
    }
    return true;
}
