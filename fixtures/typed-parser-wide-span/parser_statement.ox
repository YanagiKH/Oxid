// Experimental separate span columns; baseline grammar and continuation limits.
// Binding dispatch runs before expression parsing so assignment targets never
// allocate a Name expression. Each continuation owns its charged statement row.
fn assignment(tokens: &[i32], state: &crate::parser_state::State) -> bool {
    if tokens[state.cursor] % 64 != 2 { return false; }
    let mut next = state.cursor + 1;
    while next < state.limit && tokens[next] % 64 == 1 { next = next + 1; }
    return next < state.limit && tokens[next] % 64 == 29;
}

fn binding(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], state: &mut crate::parser_state::State) -> i32 {
    let token = tokens[state.cursor];
    crate::parser_state::bump(&*tokens, &mut *state);
    let mut tag = 6;
    if tokens[state.cursor] % 64 == 13 {
        tag = 7;
        crate::parser_state::bump(&*tokens, &mut *state);
    }
    let node = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, tag, crate::parser_state::lo(&*token_starts, token), crate::parser_state::hi(&*token_ends, token));
    if state.error != 0 { return 0; }
    let name = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 2, 25);
    if state.error != 0 { return 0; }
    crate::parser_state::low(&mut *ab, node, name);
    if tokens[state.cursor] % 64 == 26 {
        crate::parser_state::bump(&*tokens, &mut *state);
        let annotation = crate::parser_signature::ty(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
        if state.error != 0 { return 0; }
        crate::parser_state::high(&mut *ab, node, annotation);
    }
    crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 29, 26);
    return node;
}

pub fn start(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> bool {
    let token = tokens[state.cursor];
    let kind = token % 64;
    let origin = crate::parser_state::lo(&*token_starts, token);
    if kind == 45 && crate::parser_state::hi(&*token_ends, token) - origin == 5 && codes[origin] == 109 && codes[origin + 1] == 97 && codes[origin + 2] == 116 && codes[origin + 3] == 99 && codes[origin + 4] == 104 {
        state.error = 4; state.detail = 8;
        state.start = origin; state.end = crate::parser_state::hi(&*token_ends, token);
        return true;
    }
    let mut node = 0;
    let mut phase = 5;
    if kind == 12 {
        node = binding(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
    } else {
        if !assignment(&*tokens, &*state) { return false; }
        let name = crate::parser_state::bump(&*tokens, &mut *state);
        let equals = crate::parser_state::bump(&*tokens, &mut *state);
        node = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 8, origin, crate::parser_state::hi(&*token_ends, tokens[equals - 1]));
        if state.error != 0 { return true; }
        crate::parser_state::low(&mut *ab, node, name);
        crate::parser_state::high(&mut *ab, node, equals);
        phase = 6;
    }
    if state.error != 0 { return true; }
    crate::parser_state::push(&mut *frames, &mut *state, phase, node, 0);
    state.depth = 0; state.context = 0; state.floor = 0; state.mode = 1;
    return true;
}
