// One finite traversal of every body, including unreachable and untaken arms.
// Frames own distinct AST rows. Blocks add distinct scope-marker rows to the
// local bank; lookup skips those markers. Neither bank can exceed 128 rows.
fn body(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], resolved: &mut [i32], locals: &mut [i32], frames: &mut [i32], state: &mut crate::static_state::State, rows: i32, used: i32, first_local: i32) -> bool {
    let mut local_id = first_local;
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
                if kind == 5 {
                    if !crate::static_state::local_push(&mut *locals, &mut *state, node) { return false; }
                } else {
                    let target = tokens[a - 1];
                    let found = crate::static_common::find_function(&*codes, &*headers, target, rows, used);
                    if found == 0 { crate::resolver_names::fail(&mut *state, 2, target, 0); return true; }
                    resolved[node - 1] = found;
                }
                child = b;
            } else { child = headers[state.value - 1] / 4194304; }
            if child == 0 && kind == 5 {
                let mut popped = crate::static_state::local_pop(&mut *locals, &mut *state);
                while popped != node {
                    if popped == 0 { return false; }
                    popped = crate::static_state::local_pop(&mut *locals, &mut *state);
                }
            }
        } else { if kind == 6 || kind == 7 {
            if phase == 0 { child = cd[node - 1] % 256; }
            else {
                // The initializer completed before checking/inserting its name.
                let binding = crate::resolver_names::bind(&*codes, &*tokens, &*headers, &*ab, &mut *locals, &mut *state, node, rows, used);
                if binding != 1 { return binding == 2; }
                local_id = local_id + 1; resolved[node - 1] = local_id;
            }
        } else { if kind == 8 || kind == 19 {
            if phase == 0 {
                let target = tokens[a - 1];
                let found = crate::resolver_names::local(&*codes, &*tokens, &*headers, &*ab, &*locals, target, state.locals, used);
                if found == 0 { crate::resolver_names::fail(&mut *state, 1, target, 0); return true; }
                resolved[node - 1] = found;
                if kind == 8 { child = cd[node - 1] % 256; }
            }
        } else { if kind == 11 || kind == 12 {
            let mut at = state.stack - 1;
            let mut target = 0;
            while at > 0 && target == 0 {
                at = at - 1;
                let owner = frames[at] / 16;
                if headers[owner - 1] % 64 == 14 && frames[at] % 16 == 2 { target = ab[owner - 1] / 256; }
            }
            if target == 0 { crate::resolver_names::fail(&mut *state, kind - 5, headers[node - 1], 0); return true; }
            resolved[node - 1] = target;
        } else { if kind == 13 || kind == 14 {
            if phase == 0 { child = a; }
            if phase == 1 { child = b; }
            if phase == 2 && kind == 13 { child = cd[node - 1] % 256; }
        } else { if kind == 15 {
            if !crate::static_common::decimal(&*codes, tokens[a - 1], b, used, &mut *state) { crate::resolver_names::fail(&mut *state, 5, headers[node - 1], 0); return true; }
            resolved[node - 1] = state.value;
        } else {
            if kind == 9 || kind == 10 || kind >= 21 {
                if phase == 0 { child = a; }
                if phase == 1 && kind >= 24 { child = b; }
            }
        } } } } } }
        if child != 0 {
            if !crate::static_state::resume(&mut *frames, &*state, next_phase) || !crate::static_state::push(&mut *frames, &mut *state, child, 0) { return false; }
        } else {
            state.value = crate::static_state::pop(&mut *frames, &mut *state);
            if state.value == 0 { return false; }
        }
    }
    state.locals = 0; state.value = 0;
    return true;
}
pub fn run(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], resolved: &mut [i32], locals: &mut [i32], frames: &mut [i32], state: &mut crate::static_state::State, rows: i32, used: i32) -> bool {
    crate::resolver_names::preflight(&*codes, &*headers, &*ab, &*cd, &mut *resolved, &mut *state, rows, used);
    if state.stack != 0 { return true; }
    let mut i = 0;
    while i < rows {
        if headers[i] % 64 == 1 {
            let mut parameter = ab[i] / 256;
            let mut local_id = 0;
            while parameter != 0 {
                let binding = crate::resolver_names::bind(&*codes, &*tokens, &*headers, &*ab, &mut *locals, &mut *state, parameter, rows, used);
                if binding == 0 { return false; }
                if binding == 2 { break; }
                local_id = local_id + 1; resolved[parameter - 1] = local_id;
                parameter = headers[parameter - 1] / 4194304;
            }
            if parameter == 0 {
                if !crate::static_state::push(&mut *frames, &mut *state, cd[i] / 256, 0) { return false; }
                if !body(&*codes, &*tokens, &*headers, &*ab, &*cd, &mut *resolved, &mut *locals, &mut *frames, &mut *state, rows, used, local_id) { return false; }
            }
            // Finish every traversal bank before handing State to the emitter.
            let mut clear = 0;
            while clear < 129 { locals[clear] = 0; frames[clear] = 0; clear = clear + 1; }
            if state.stack != 0 { return true; }
        }
        i = i + 1;
    }
    return true;
}
