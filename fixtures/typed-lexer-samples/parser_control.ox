// If phases 8/9/10 await condition/then/else; While phases 11/12 await
// condition/body. Their frame aux saves the enclosing active block depth.
fn body(codes: &[i32], tokens: &[i32], headers: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, message: i32) -> () {
    let depth = frames[state.stack - 1] % 65536 / 32;
    let open = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 24, message);
    if state.error != 0 { return; }
    let token = tokens[open - 1];
    let block = crate::parser_state::row(&mut *headers, &mut *state, 5, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::push(&mut *frames, &mut *state, 4, block, depth + 1);
    state.mode = 0;
    return;
}

pub fn start(tokens: &[i32], headers: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let depth = frames[state.stack - 1] % 65536 / 32;
    let token = tokens[state.cursor];
    crate::parser_state::bump(&*tokens, &mut *state);
    let mut tag = 13;
    let mut phase = 8;
    if token % 64 == 18 { tag = 14; phase = 11; }
    let node = crate::parser_state::row(&mut *headers, &mut *state, tag, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::push(&mut *frames, &mut *state, phase, node, depth);
    state.depth = 0; state.context = 1; state.floor = 0; state.mode = 1;
    return;
}

pub fn condition_done(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    let depth = frame % 65536 / 32;
    crate::parser_state::low(&mut *ab, frame / 65536, state.value);
    // Parse the whole condition first; then gate depth before requiring '{'.
    if depth >= 64 {
        let token = tokens[state.cursor];
        state.error = 3; state.detail = 2;
        state.start = crate::parser_state::lo(token);
        state.end = crate::parser_state::hi(token);
        return;
    }
    let mut message = 27;
    if frame % 32 == 11 { message = 29; }
    frames[state.stack - 1] = frame + 1;
    body(&*codes, &*tokens, &mut *headers, &mut *frames, &mut *state, message);
    return;
}

// True returns one complete statement for the driver to link after this frame
// is popped. An else reuses the same statement frame and starts its child block.
pub fn body_done(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> bool {
    let frame = frames[state.stack - 1];
    let phase = frame % 32;
    if phase != 9 && phase != 10 && phase != 12 { state.error = 9; return false; }
    let node = frame / 65536;
    let block = state.value;
    if phase == 10 { crate::parser_state::low(&mut *cd, node, block); }
    else { crate::parser_state::high(&mut *ab, node, block); }
    if phase == 9 && tokens[state.cursor] % 64 == 19 {
        crate::parser_state::bump(&*tokens, &mut *state);
        frames[state.stack - 1] = frame + 1;
        body(&*codes, &*tokens, &mut *headers, &mut *frames, &mut *state, 28);
        return false;
    }
    let origin = crate::parser_state::lo(headers[node - 1]);
    let end = crate::parser_state::hi(headers[block - 1]);
    crate::parser_state::span(&mut *headers, node, origin, end);
    let finished = crate::parser_state::pop(&mut *frames, &mut *state);
    state.value = node; state.mode = 0;
    return true;
}
