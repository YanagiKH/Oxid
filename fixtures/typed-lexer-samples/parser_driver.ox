// First real dispatcher. Error 5 explicitly marks planned grammar not wired yet.
fn link_statement(headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], block_id: i32, statement: i32) -> () {
    let tail = cd[block_id - 1] % 256;
    if tail == 0 { crate::parser_state::high(&mut *ab, block_id, statement); }
    else { crate::parser_state::link(&mut *headers, tail, statement); }
    crate::parser_state::low(&mut *cd, block_id, statement);
    return;
}
fn family(codes: &[i32], token: i32) -> i32 {
    let kind = token % 64;
    if kind == 7 { return 1; }
    if kind == 8 { return 2; }
    if kind == 6 { return 3; }
    let at = crate::parser_state::lo(token);
    if kind == 45 && crate::parser_state::hi(token) - at == 4 && codes[at] == 101 && codes[at + 1] == 110 && codes[at + 2] == 117 && codes[at + 3] == 109 { return 4; }
    return 0;
}
fn root(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let token = tokens[state.cursor];
    let kind = token % 64;
    if kind == 47 { state.mode = 9; return; }
    let mut rejected = family(&*codes, token);
    let mut starter = token;
    if kind == 9 {
        let mut next = state.cursor + 1;
        while next < state.limit && tokens[next] % 64 == 1 { next = next + 1; }
        let candidate = family(&*codes, tokens[next]);
        if candidate == 1 || candidate == 3 || candidate == 4 { rejected = candidate; starter = tokens[next]; }
    }
    if rejected != 0 {
        state.error = 4; state.detail = rejected;
        state.start = crate::parser_state::lo(starter); state.end = crate::parser_state::hi(starter);
        return;
    }
    crate::parser_signature::start(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
    return;
}
fn block(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    let block_id = frame / 65536;
    let token = tokens[state.cursor];
    let kind = token % 64;
    if kind == 25 {
        let close = crate::parser_state::bump(&*tokens, &mut *state);
        let origin = crate::parser_state::lo(headers[block_id - 1]);
        crate::parser_state::span(&mut *headers, block_id, origin, crate::parser_state::hi(token));
        crate::parser_state::low(&mut *ab, block_id, close);
        crate::parser_state::low(&mut *cd, block_id, 0);
        let finished = crate::parser_state::pop(&mut *frames, &mut *state);
        state.value = block_id; state.mode = 3;
        return;
    }
    if kind == 47 { crate::parser_state::fail(&*codes, &*tokens, &mut *state, 11); return; }
    if kind == 14 || kind == 15 || kind == 16 {
        let at = crate::parser_state::bump(&*tokens, &mut *state);
        let tag = kind - 4;
        let statement = crate::parser_state::row(&mut *headers, &mut *state, tag, crate::parser_state::lo(token), crate::parser_state::hi(token));
        if state.error != 0 { return; }
        if kind != 14 && tokens[state.cursor] % 64 != 28 {
            state.error = 1; state.detail = 21;
            state.start = crate::parser_state::lo(tokens[state.cursor]); state.end = crate::parser_state::hi(tokens[state.cursor]);
            return;
        }
        if tokens[state.cursor] % 64 == 28 {
            let semi = crate::parser_state::bump(&*tokens, &mut *state);
            crate::parser_state::span(&mut *headers, statement, crate::parser_state::lo(token), crate::parser_state::hi(tokens[semi - 1]));
            link_statement(&mut *headers, &mut *ab, &mut *cd, block_id, statement);
            return;
        }
        crate::parser_state::push(&mut *frames, &mut *state, 7, statement, 0);
    } else {
        if kind == 12 || kind == 17 || kind == 18 {
            state.error = 5; state.detail = 100 + kind;
            state.start = crate::parser_state::lo(token); state.end = crate::parser_state::hi(token);
            return;
        }
        // Reuse this Block frame; ExprStmt is allocated only after its semicolon.
        frames[state.stack - 1] = frame - 4 + 18;
    }
    state.depth = 0; state.context = 0; state.floor = 0; state.mode = 1;
    return;
}
fn expression_done(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let token = tokens[state.cursor];
    let kind = token % 64;
    if kind == 40 || kind == 41 || kind == 42 || kind == 43 || kind == 44 || (kind >= 30 && kind <= 38) {
        state.error = 5; state.detail = 24;
        state.start = crate::parser_state::lo(token); state.end = crate::parser_state::hi(token);
        return;
    }
    let semi = crate::parser_state::expect(&*codes, &*tokens, &mut *state, 28, 17);
    if state.error != 0 { return; }
    let frame = frames[state.stack - 1];
    let phase = frame % 32;
    let node = frame / 65536;
    let value = state.value;
    if phase == 7 {
        crate::parser_state::low(&mut *ab, node, value);
        let origin = crate::parser_state::lo(headers[node - 1]);
        crate::parser_state::span(&mut *headers, node, origin, crate::parser_state::hi(tokens[semi - 1]));
        let finished = crate::parser_state::pop(&mut *frames, &mut *state);
        link_statement(&mut *headers, &mut *ab, &mut *cd, frames[state.stack - 1] / 65536, node);
    } else {
        if phase != 18 { state.error = 9; return; }
        let origin = crate::parser_state::lo(headers[value - 1]);
        let statement = crate::parser_state::row(&mut *headers, &mut *state, 9, origin, crate::parser_state::hi(tokens[semi - 1]));
        if state.error != 0 { return; }
        crate::parser_state::low(&mut *ab, statement, value);
        frames[state.stack - 1] = frame - 18 + 4;
        link_statement(&mut *headers, &mut *ab, &mut *cd, node, statement);
    }
    state.mode = 0;
    return;
}
fn block_done(headers: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    if frame % 32 != 3 { state.error = 9; return; }
    let function = frame / 65536;
    crate::parser_state::high(&mut *cd, function, state.value);
    let finished = crate::parser_state::pop(&mut *frames, &mut *state);
    if state.tail == 0 { state.items = function; }
    else { crate::parser_state::link(&mut *headers, state.tail, function); }
    state.tail = function; state.mode = 0;
    return;
}
pub fn parse(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    frames[0] = 1; state.stack = 1;
    while tokens[state.cursor] % 64 == 1 { state.cursor = state.cursor + 1; }
    while state.error == 0 && state.mode != 9 {
        if state.mode == 1 {
            crate::parser_atom::parse(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *state);
        } else {
            if state.mode == 2 {
                expression_done(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
            } else {
                if state.mode == 3 { block_done(&mut *headers, &mut *cd, &mut *frames, &mut *state); }
                else {
                    if frames[state.stack - 1] % 32 == 1 { root(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state); }
                    else { block(&*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state); }
                }
            }
        }
    }
    return;
}
