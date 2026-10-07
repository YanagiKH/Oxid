// A Call charges only its callee and reuses one frame for every argument.
// Phase 15 awaits an argument; phase 19 requires comma or closing parenthesis.
fn argument(tokens: &[i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    crate::parser_state::restore_expression(&mut *state, frame);
    state.depth = state.depth + 1;
    state.context = 0;
    state.floor = 0;
    frames[state.stack - 1] = frame - 19 + 15;
    state.mode = 1;
    let token = tokens[state.cursor];
    // The canonical argument-entry depth gate wins over borrow refusal.
    if state.depth >= 64 {
        state.error = 3; state.detail = 1;
    } else {
        if token % 64 == 10 { state.error = 4; state.detail = 14; }
    }
    if state.error != 0 {
        state.start = crate::parser_state::lo(token);
        state.end = crate::parser_state::hi(token);
    }
    return;
}

pub fn start(tokens: &[i32], headers: &mut [i32], ab: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, callee: i32) -> () {
    let token = tokens[callee - 1];
    let node = crate::parser_state::row(&mut *headers, &mut *state, 20, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *ab, node, callee);
    crate::parser_state::bump(&*tokens, &mut *state);
    let aux = crate::parser_state::expression_aux(&*state);
    crate::parser_state::push(&mut *frames, &mut *state, 19, node, aux);
    if state.error != 0 { return; }
    state.mode = 2;
    if tokens[state.cursor] % 64 != 23 {
        argument(&*tokens, &mut *frames, &mut *state);
    }
    return;
}

pub fn argument_done(headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    let node = frame / 65536;
    let value = state.value;
    let tail = cd[node - 1] % 256;
    if tail == 0 { crate::parser_state::high(&mut *ab, node, value); }
    else { crate::parser_state::link(&mut *headers, tail, value); }
    crate::parser_state::low(&mut *cd, node, value);
    let height = cd[value - 1] / 256;
    if height > cd[node - 1] / 256 { crate::parser_state::high(&mut *cd, node, height); }
    frames[state.stack - 1] = frame - 15 + 19;
    return;
}

pub fn resume(codes: &[i32], tokens: &[i32], headers: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    if tokens[state.cursor] % 64 == 27 {
        crate::parser_state::bump(&*tokens, &mut *state);
        argument(&*tokens, &mut *frames, &mut *state);
        return;
    }
    let close = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 23, 24);
    if state.error != 0 { return; }
    crate::parser_atom::postfix(&*codes, &*tokens, &mut *state);
    if state.error != 0 { return; }
    let frame = frames[state.stack - 1];
    let node = frame / 65536;
    let origin = crate::parser_state::lo(headers[node - 1]);
    let end = crate::parser_state::hi(tokens[close - 1]);
    let height = cd[node - 1] / 256 + 1;
    crate::parser_state::span(&mut *headers, node, origin, end);
    if height > 64 {
        state.error = 3; state.detail = 1;
        state.start = origin; state.end = end;
        return;
    }
    crate::parser_state::low(&mut *cd, node, 0);
    crate::parser_state::high(&mut *cd, node, height);
    let finished = crate::parser_state::pop(&mut *frames, &mut *state);
    crate::parser_state::restore_expression(&mut *state, finished);
    state.value = node;
    return;
}
