// Experimental separate span columns; baseline grammar and continuation limits.
// Scalar grammar dispatcher. Helpers return transitions to this acyclic driver.
fn link_statement(headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], block_id: i32, statement: i32) -> () {
    let tail = cd[block_id - 1] % 256;
    if tail == 0 { crate::parser_state::high(&mut *ab, block_id, statement); }
    else { crate::parser_state::link(&mut *headers, tail, statement); }
    crate::parser_state::low(&mut *cd, block_id, statement);
    return;
}
fn family(token_starts: &[i32], token_ends: &[i32], codes: &[i32], token: i32) -> i32 {
    let kind = token % 64;
    if kind == 7 { return 1; }
    if kind == 8 { return 2; }
    if kind == 6 { return 3; }
    let at = crate::parser_state::lo(&*token_starts, token);
    if kind == 45 && crate::parser_state::hi(&*token_ends, token) - at == 4 && crate::source_bytes::at(&*codes, at) == 101 && crate::source_bytes::at(&*codes, at + 1) == 110 && crate::source_bytes::at(&*codes, at + 2) == 117 && crate::source_bytes::at(&*codes, at + 3) == 109 { return 4; }
    return 0;
}
fn root(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let token = tokens[state.cursor];
    let kind = token % 64;
    if kind == 47 { state.mode = 9; return; }
    let mut rejected = family(&*token_starts, &*token_ends, &*codes, token);
    let mut starter = token;
    if kind == 9 {
        let mut next = state.cursor + 1;
        while next < state.limit && tokens[next] % 64 == 1 { next = next + 1; }
        let candidate = family(&*token_starts, &*token_ends, &*codes, tokens[next]);
        if candidate == 1 || candidate == 3 || candidate == 4 { rejected = candidate; starter = tokens[next]; }
        else {
            // Invalid public item starts fail at the unconsumed original pub.
            if tokens[next] % 64 != 5 {
                crate::parser_state::fail(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 1);
                return;
            }
        }
    }
    if rejected != 0 {
        state.error = 4; state.detail = rejected;
        state.start = crate::parser_state::lo(&*token_starts, starter); state.end = crate::parser_state::hi(&*token_ends, starter);
        return;
    }
    crate::parser_signature::start(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
    return;
}
fn block(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    let block_id = frame / 65536;
    let token = tokens[state.cursor];
    let kind = token % 64;
    if kind == 25 {
        let close = crate::parser_state::bump(&*tokens, &mut *state);
        let origin = row_starts[block_id - 1];
        crate::parser_state::span(&mut *row_starts, &mut *row_ends, block_id, origin, crate::parser_state::hi(&*token_ends, token));
        crate::parser_state::low(&mut *ab, block_id, close);
        crate::parser_state::low(&mut *cd, block_id, 0);
        let finished = crate::parser_state::pop(&mut *frames, &mut *state);
        state.value = block_id; state.mode = 3;
        return;
    }
    if kind == 47 { crate::parser_state::fail(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 11); return; }
    let binding = crate::parser_statement::start(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *frames, &mut *state);
    if binding { return; }
    if kind == 17 || kind == 18 {
        crate::parser_control::start(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*tokens, &mut *headers, &mut *frames, &mut *state);
        return;
    }
    if kind == 14 || kind == 15 || kind == 16 {
        let at = crate::parser_state::bump(&*tokens, &mut *state);
        let tag = kind - 4;
        let statement = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, tag, crate::parser_state::lo(&*token_starts, token), crate::parser_state::hi(&*token_ends, token));
        if state.error != 0 { return; }
        if kind != 14 && tokens[state.cursor] % 64 != 28 {
            state.error = 1; state.detail = 21;
            state.start = crate::parser_state::lo(&*token_starts, tokens[state.cursor]); state.end = crate::parser_state::hi(&*token_ends, tokens[state.cursor]);
            return;
        }
        if tokens[state.cursor] % 64 == 28 {
            let semi = crate::parser_state::bump(&*tokens, &mut *state);
            crate::parser_state::span(&mut *row_starts, &mut *row_ends, statement, crate::parser_state::lo(&*token_starts, token), crate::parser_state::hi(&*token_ends, tokens[semi - 1]));
            link_statement(&mut *headers, &mut *ab, &mut *cd, block_id, statement);
            return;
        }
        crate::parser_state::push(&mut *frames, &mut *state, 7, statement, 0);
    } else {
        // Reuse this Block frame; ExprStmt is allocated only after its semicolon.
        frames[state.stack - 1] = frame - 4 + 18;
    }
    state.depth = 0; state.context = 0; state.floor = 0; state.mode = 1;
    return;
}
fn expression_done(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    let phase = frame % 32;
    if phase == 8 || phase == 11 {
        crate::parser_control::condition_done(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *frames, &mut *state);
        return;
    }
    let semi = crate::parser_state::expect(&*token_starts, &*token_ends, &*codes, &*tokens, &mut *state, 28, 17);
    if state.error != 0 { return; }
    let node = frame / 65536;
    let value = state.value;
    if phase == 5 || phase == 6 || phase == 7 {
        if phase == 7 { crate::parser_state::low(&mut *ab, node, value); }
        else { crate::parser_state::low(&mut *cd, node, value); }
        let origin = row_starts[node - 1];
        crate::parser_state::span(&mut *row_starts, &mut *row_ends, node, origin, crate::parser_state::hi(&*token_ends, tokens[semi - 1]));
        let finished = crate::parser_state::pop(&mut *frames, &mut *state);
        link_statement(&mut *headers, &mut *ab, &mut *cd, frames[state.stack - 1] / 65536, node);
    } else {
        if phase != 18 { state.error = 9; return; }
        let origin = row_starts[value - 1];
        let statement = crate::parser_state::row(&mut *row_starts, &mut *row_ends, &mut *headers, &mut *state, 9, origin, crate::parser_state::hi(&*token_ends, tokens[semi - 1]));
        if state.error != 0 { return; }
        crate::parser_state::low(&mut *ab, statement, value);
        frames[state.stack - 1] = frame - 18 + 4;
        link_statement(&mut *headers, &mut *ab, &mut *cd, node, statement);
    }
    state.mode = 0;
    return;
}
fn block_done(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let frame = frames[state.stack - 1];
    if frame % 32 != 3 {
        let completed = crate::parser_control::body_done(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
        if completed && state.error == 0 {
            link_statement(&mut *headers, &mut *ab, &mut *cd, frames[state.stack - 1] / 65536, state.value);
        }
        return;
    }
    let function = frame / 65536;
    crate::parser_state::high(&mut *cd, function, state.value);
    let finished = crate::parser_state::pop(&mut *frames, &mut *state);
    if state.tail == 0 { state.items = function; }
    else { crate::parser_state::link(&mut *headers, state.tail, function); }
    state.tail = function; state.mode = 0;
    return;
}
pub fn parse(token_starts: &[i32], token_ends: &[i32], row_starts: &mut [i32], row_ends: &mut [i32], codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    frames[0] = 1; state.stack = 1;
    while tokens[state.cursor] % 64 == 1 { state.cursor = state.cursor + 1; }
    while state.error == 0 && state.mode != 9 {
        if state.mode == 1 {
            crate::parser_atom::parse(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
        } else {
            if state.mode == 2 {
                let returned = crate::parser_expression::resume(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
                if returned && state.error == 0 {
                    expression_done(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state);
                }
            } else {
                if state.mode == 3 { block_done(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state); }
                else {
                    if frames[state.stack - 1] % 32 == 1 { root(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state); }
                    else { block(&*token_starts, &*token_ends, &mut *row_starts, &mut *row_ends, &*codes, &*tokens, &mut *headers, &mut *ab, &mut *cd, &mut *frames, &mut *state); }
                }
            }
        }
    }
    return;
}
