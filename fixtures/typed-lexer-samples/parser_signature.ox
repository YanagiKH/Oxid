// Function headers and atomic types use acyclic helpers and charged rows.
fn refuse(tokens: &[i32], state: &mut crate::parser_state::State, family: i32) -> () {
    let token = tokens[state.cursor];
    state.error = 4;
    state.detail = family;
    state.start = crate::parser_state::lo(token);
    state.end = crate::parser_state::hi(token);
    return;
}

fn qualified(tokens: &[i32], state: &mut crate::parser_state::State) -> () {
    if tokens[state.cursor] % 64 == 26 && state.cursor + 1 < state.limit {
        let first = tokens[state.cursor];
        let second = tokens[state.cursor + 1];
        if second % 64 == 26 && crate::parser_state::hi(first) == crate::parser_state::lo(second) {
            refuse(&*tokens, &mut *state, 7);
        }
    }
    return;
}

pub fn ty(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], state: &mut crate::parser_state::State) -> i32 {
    let token = tokens[state.cursor];
    let start = crate::parser_state::lo(token);
    if token % 64 == 45 && crate::parser_state::hi(token) == start + 1 && codes[start] == 91 {
        refuse(&*tokens, &mut *state, 6);
        return 0;
    }
    if token % 64 == 22 {
        crate::parser_state::bump(&*tokens, &mut *state);
        let node = crate::parser_state::row(&mut *headers, &mut *state, 4, start, crate::parser_state::hi(token));
        if state.error != 0 { return 0; }
        let close = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 23, 6);
        if state.error != 0 { return 0; }
        crate::parser_state::span(&mut *headers, node, start, crate::parser_state::hi(tokens[close - 1]));
        return node;
    }
    let name = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 2, 5);
    if state.error != 0 { return 0; }
    let node = crate::parser_state::row(&mut *headers, &mut *state, 3, start, crate::parser_state::hi(token));
    if state.error != 0 { return 0; }
    crate::parser_state::low(&mut *ab, node, name);
    qualified(&*tokens, &mut *state);
    if state.error != 0 { return 0; }
    return node;
}

fn parameter(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], state: &mut crate::parser_state::State) -> i32 {
    let name = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 2, 3);
    if state.error != 0 { return 0; }
    let token = tokens[name - 1];
    let node = crate::parser_state::row(&mut *headers, &mut *state, 2, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return 0; }
    crate::parser_state::expect(&*codes, &*tokens, &mut *state, 26, 4);
    if state.error != 0 { return 0; }
    if tokens[state.cursor] % 64 == 10 {
        refuse(&*tokens, &mut *state, 5);
        return 0;
    }
    let type_node = ty(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
    if state.error != 0 { return 0; }
    crate::parser_state::low(&mut *ab, node, type_node);
    return node;
}

fn parameters(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], state: &mut crate::parser_state::State, function: i32) -> () {
    if tokens[state.cursor] % 64 != 23 {
        while state.error == 0 {
            let node = parameter(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
            if state.error != 0 { return; }
            let tail = cd[function - 1] / 256;
            if tail == 0 { crate::parser_state::high(&mut *ab, function, node); }
            else { crate::parser_state::link(&mut *headers, tail, node); }
            crate::parser_state::high(&mut *cd, function, node);
            if tokens[state.cursor] % 64 != 27 { break; }
            crate::parser_state::bump(&*tokens, &mut *state);
        }
    }
    crate::parser_state::expect(&*codes, &*tokens, &mut *state, 23, 8);
    if state.error != 0 { return; }
    crate::parser_state::high(&mut *cd, function, 0);
    return;
}

fn body(codes: &[i32], tokens: &[i32], headers: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, function: i32) -> () {
    let open = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 24, 10);
    if state.error != 0 { return; }
    let token = tokens[open - 1];
    let block = crate::parser_state::row(&mut *headers, &mut *state, 5, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::push(&mut *frames, &mut *state, 3, function, 0);
    if state.error != 0 { return; }
    crate::parser_state::push(&mut *frames, &mut *state, 4, block, 1);
    if state.error != 0 { return; }
    state.mode = 0;
    return;
}

fn result_and_body(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State, function: i32) -> () {
    crate::parser_state::expect(&*codes, &*tokens, &mut *state, 39, 9);
    if state.error != 0 { return; }
    let result = ty(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *state);
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *cd, function, result);
    body(&*codes, &*tokens, &mut *headers, &mut *frames, &mut *state, function);
    return;
}

pub fn start(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let mut public = 0;
    if tokens[state.cursor] % 64 == 9 { public = crate::parser_state::bump(&*tokens, &mut *state); }
    let keyword = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 5, 1);
    if state.error != 0 { return; }
    let token = tokens[keyword - 1];
    let function = crate::parser_state::row(&mut *headers, &mut *state, 1, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *ab, function, public);
    let name = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 2, 2);
    if state.error != 0 { return; }
    crate::parser_state::span(&mut *headers, function, crate::parser_state::lo(tokens[name - 1]), crate::parser_state::hi(tokens[name - 1]));
    crate::parser_state::expect(&*codes, &*tokens, &mut *state, 22, 7);
    if state.error != 0 { return; }
    parameters(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *state, function);
    if state.error != 0 { return; }
    result_and_body(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state, function);
    return;
}
