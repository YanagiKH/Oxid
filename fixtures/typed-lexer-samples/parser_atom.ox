// Primary/prefix entry. Calls retain an explicit stage-pending result.
fn reject(state: &mut crate::parser_state::State, error: i32, detail: i32, token: i32) -> () {
    state.error = error;
    state.detail = detail;
    state.start = crate::parser_state::lo(token);
    state.end = crate::parser_state::hi(token);
    return;
}

fn bracket(codes: &[i32], token: i32) -> bool {
    let start = crate::parser_state::lo(token);
    return token % 64 == 45 && crate::parser_state::hi(token) == start + 1 && codes[start] == 91;
}

// This check uses adjacent token slots as well as adjacent byte spans. Trivia
// between the colons cannot form the canonical double-colon punctuation.
fn qualified(tokens: &[i32], state: &crate::parser_state::State) -> bool {
    if tokens[state.cursor] % 64 != 26 || state.cursor + 1 >= state.limit { return false; }
    let first = tokens[state.cursor];
    let second = tokens[state.cursor + 1];
    return second % 64 == 26 && crate::parser_state::hi(first) == crate::parser_state::lo(second);
}

fn name_tail(codes: &[i32], tokens: &[i32], state: &mut crate::parser_state::State) -> () {
    let token = tokens[state.cursor];
    let kind = token % 64;
    if qualified(&*tokens, &*state) {
        reject(&mut *state, 4, 10, token);
        return;
    }
    if kind == 22 {
        reject(&mut *state, 5, 20, token);
        return;
    }
    if kind == 11 {
        reject(&mut *state, 4, 11, token);
        return;
    }
    if bracket(&*codes, token) {
        reject(&mut *state, 4, 12, token);
        return;
    }
    if kind == 24 && state.context == 0 {
        reject(&mut *state, 4, 13, token);
    }
    return;
}

pub fn postfix(codes: &[i32], tokens: &[i32], state: &mut crate::parser_state::State) -> () {
    let trailing = tokens[state.cursor];
    if trailing % 64 == 11 || bracket(&*codes, trailing) {
        // Canonical arrays.rs checks this before primary height/construction.
        reject(&mut *state, 2, 3, trailing);
    }
    return;
}

fn signed_number(tokens: &[i32], state: &crate::parser_state::State) -> bool {
    let mut next = state.cursor + 1;
    while next < state.limit && tokens[next] % 64 == 1 { next = next + 1; }
    return next < state.limit && tokens[next] % 64 == 3;
}

fn prefix(tokens: &[i32], headers: &mut [i32], ab: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let token = tokens[state.cursor];
    let operator = crate::parser_state::bump(&*tokens, &mut *state);
    let mut tag = 23;
    if token % 64 == 40 { tag = 22; }
    let node = crate::parser_state::row(&mut *headers, &mut *state, tag, crate::parser_state::lo(token), crate::parser_state::hi(token));
    if state.error != 0 { return; }
    crate::parser_state::high(&mut *ab, node, operator);
    let aux = crate::parser_state::expression_aux(&*state);
    crate::parser_state::push(&mut *frames, &mut *state, 13, node, aux);
    state.depth = state.depth + 1;
    state.floor = 6;
    return;
}

pub fn parse(codes: &[i32], tokens: &[i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], frames: &mut [i32], state: &mut crate::parser_state::State) -> () {
    let token = tokens[state.cursor];
    let kind = token % 64;
    // The canonical depth gate precedes primary dispatch and family refusal.
    if state.depth >= 64 {
        reject(&mut *state, 3, 1, token);
        return;
    }
    if kind == 32 || (kind == 40 && !signed_number(&*tokens, &*state)) {
        prefix(&*tokens, &mut *headers, &mut *ab, &mut *frames, &mut *state);
        return;
    }
    let start = crate::parser_state::lo(token);
    let mut end = crate::parser_state::hi(token);
    let mut tag = 0;
    let mut payload = 0;
    let mut negative = 0;
    if kind == 20 || kind == 21 {
        tag = 16;
        if kind == 20 { tag = 17; }
        crate::parser_state::bump(&*tokens, &mut *state);
    } else { if kind == 3 || kind == 40 {
        tag = 15;
        if kind == 40 {
            negative = 1;
            crate::parser_state::bump(&*tokens, &mut *state);
        }
        let digits = tokens[state.cursor];
        payload = crate::parser_state::bump(&*tokens, &mut *state);
        end = crate::parser_state::hi(digits);
        let mut position = crate::parser_state::lo(digits);
        while position < end {
            if codes[position] < 48 || codes[position] > 57 {
                state.error = 2;
                state.detail = 2;
                state.start = start;
                state.end = end;
                return;
            }
            position = position + 1;
        }
    } else { if kind == 2 {
        tag = 19;
        payload = crate::parser_state::bump(&*tokens, &mut *state);
        name_tail(&*codes, &*tokens, &mut *state);
        if state.error != 0 { return; }
    } else { if kind == 22 {
        crate::parser_state::bump(&*tokens, &mut *state);
        if tokens[state.cursor] % 64 != 23 {
            let group = crate::parser_state::row(&mut *headers, &mut *state, 21, start, end);
            if state.error != 0 { return; }
            let aux = crate::parser_state::expression_aux(&*state);
            crate::parser_state::push(&mut *frames, &mut *state, 14, group, aux);
            state.depth = state.depth + 1;
            state.context = 0;
            state.floor = 0;
            return;
        }
        tag = 18;
        end = crate::parser_state::hi(tokens[state.cursor]);
        crate::parser_state::bump(&*tokens, &mut *state);
    } else { if bracket(&*codes, token) {
        reject(&mut *state, 4, 9, token);
        return;
    } else {
        crate::parser_state::fail(&*codes, &*tokens, &mut *state, 13);
        return;
    }
    } } } }
    postfix(&*codes, &*tokens, &mut *state);
    if state.error != 0 { return; }
    let node = crate::parser_state::row(&mut *headers, &mut *state, tag, start, end);
    if state.error != 0 { return; }
    crate::parser_state::low(&mut *ab, node, payload);
    crate::parser_state::high(&mut *ab, node, negative);
    crate::parser_state::high(&mut *cd, node, 1);
    state.value = node;
    state.mode = 2;
    return;
}
