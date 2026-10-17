// Experimental separate span columns; baseline grammar and continuation limits.
// Function headers and atomic types use acyclic helpers and charged rows.
fn refuse(token_starts: &[i32], token_ends: &[i32], tokens: &[i32], state: &mut crate::parser_state::State, family: i32) -> () {
    let token = tokens[state.cursor];
    state.error = 4;
    state.detail = family;
    state.start = crate::parser_state::lo(&*token_starts, token);
    state.end = crate::parser_state::hi(&*token_ends, token);
    return;
}

fn qualified(token_starts: &[i32], token_ends: &[i32], tokens: &[i32], state: &mut crate::parser_state::State) -> () {
    if tokens[state.cursor] % 64 == 26 && state.cursor + 1 < state.limit {
        let first = tokens[state.cursor];
        let second = tokens[state.cursor + 1];
        if second % 64 == 26 && crate::parser_state::hi(&*token_ends, first) == crate::parser_state::lo(&*token_starts, second) {
            refuse(&*token_starts, &*token_ends, &*tokens, &mut *state, 7);
        }
    }
    return;
}

pub fn ty(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], state: &mut crate::parser_state::State) -> i32 {
    let token = tokens[state.cursor];
    let origin = crate::parser_state::lo(&*token_starts, token);
    if token % 64 == 45 && crate::parser_state::hi(&*token_ends, token) == origin + 1 && codes[origin] == 91 {
        refuse(&*token_starts, &*token_ends, &*tokens, &mut *state, 6);
        return 0;
    }
    if token % 64 == 22 {
        crate::parser_state::bump(&*tokens, &mut *state);
        let node = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 4, origin, crate::parser_state::hi(&*token_ends, token));
        if state.error != 0 { return 0; }
        let close = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 23, 6);
        if state.error != 0 { return 0; }
        crate::parser_state::span(&mut *row_starts, &mut *row_ends, node, origin, crate::parser_state::hi(&*token_ends, tokens[close - 1]));
        return node;
    }
    let name = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 2, 5);
    if state.error != 0 { return 0; }
    let node = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 3, origin, crate::parser_state::hi(&*token_ends, token));
    if state.error != 0 { return 0; }
    crate::parser_state::low(&mut *ab, node, name);
    qualified(&*token_starts, &*token_ends, &*tokens, &mut *state);
    if state.error != 0 { return 0; }
    return node;
}

fn parameter(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], state: &mut crate::parser_state::State) -> i32 {
    let name = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 2, 3);
    if state.error != 0 { return 0; }
    let token = tokens[name - 1];
    let node = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 2, crate::parser_state::lo(&*token_starts, token), crate::parser_state::hi(&*token_ends, token));
    if state.error != 0 { return 0; }
    crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 26, 4);
    if state.error != 0 { return 0; }
    if tokens[state.cursor] % 64 == 10 {
        refuse(&*token_starts, &*token_ends, &*tokens, &mut *state, 5);
        return 0;
    }
    let type_node = ty(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
    if state.error != 0 { return 0; }
    crate::parser_state::low(&mut *ab, node, type_node);
    return node;
}

fn parameters(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], state: &mut crate::parser_state::State, function: i32) -> () {
    if tokens[state.cursor] % 64 != 23 {
        while state.error == 0 {
            let node = parameter(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
            if state.error != 0 { return; }
            let tail = cd[function - 1] / 256;
            if tail == 0 { crate::parser_state::high(&mut *ab, function, node); }
            else { crate::parser_state::link(&mut *headers, tail, node); }
            crate::parser_state::high(&mut *cd, function, node);
            if tokens[state.cursor] % 64 != 27 { break; }
            crate::parser_state::bump(&*tokens, &mut *state);
        }
    }
    crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 23, 8);
    if state.error != 0 { return; }
    crate::parser_state::high(&mut *cd, function, 0);
    return;
}

fn body(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, function: i32) -> () {
    let open = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 24, 10);
    if state.error != 0 { return; }
    let token = tokens[open - 1];
    let block = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 5, crate::parser_state::lo(&*token_starts, token), crate::parser_state::hi(&*token_ends, token));
    if state.error != 0 { return; }
    crate::parser_state::push(&mut *frames, &mut *state, 3, function, 0);
    if state.error != 0 { return; }
    crate::parser_state::push(&mut *frames, &mut *state, 4, block, 1);
    if state.error != 0 { return; }
    state.mode = 0;
    return;
}

fn result_and_body(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, function: i32) -> () {
    crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 39, 9);
    if state.error != 0 { return; }
    let result = ty(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *cd, function, result);
    body(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *frames, &mut *state, function);
    return;
}

pub fn start(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let mut public = 0;
    if tokens[state.cursor] % 64 == 9 { public = crate::parser_state::bump(&*tokens, &mut *state); }
    let keyword = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 5, 1);
    if state.error != 0 { return; }
    let token = tokens[keyword - 1];
    let function = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 1, crate::parser_state::lo(&*token_starts, token), crate::parser_state::hi(&*token_ends, token));
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *ab, function, public);
    let name = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 2, 2);
    if state.error != 0 { return; }
    crate::parser_state::span(&mut *row_starts, &mut *row_ends, function, crate::parser_state::lo(&*token_starts, tokens[name - 1]), crate::parser_state::hi(&*token_ends, tokens[name - 1]));
    crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 22, 7);
    if state.error != 0 { return; }
    parameters(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *state, function);
    if state.error != 0 { return; }
    result_and_body(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state, function);
    return;
}
