// Synthetic carrier exercises plus real AST name/literal observations only.
// The observations below are NOT resolution or type-checking facts.
pub fn union(left: i32, right: i32) -> i32 {
    let mut bit = 1;
    let mut mask = 0;
    while bit < 16 {
        if left / bit % 2 == 1 || right / bit % 2 == 1 { mask = mask + bit; }
        bit = bit * 2;
    }
    return mask;
}
pub fn sequence(first: i32, next: i32) -> i32 {
    if first % 2 == 0 { return first; }
    return union(first - 1, next);
}
pub fn loop_flow(body: i32) -> i32 { return 1 + 2 * (body / 2 % 2); }
pub fn run(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], resolved: &mut [i32], semantic: &mut [i32], locals: &mut [i32], frames: &mut [i32], rows: i32, used: i32) -> bool {
    let mut state = crate::static_state::State { stack: 0, locals: 0, value: 0 };
    let mut i = 0;
    while i < 128 {
        resolved[i] = i - 64;
        let mask = i % 16;
        semantic[i] = union(mask, 15 - mask) + 16 * sequence(mask, 9) + 256 * loop_flow(mask);
        if !crate::static_state::push(&mut *frames, &mut state, i + 1, mask) || !crate::static_state::local_push(&mut *locals, &mut state, i + 1) { return false; }
        i = i + 1;
    }
    resolved[0] = -2147483648; resolved[127] = 2147483647;
    if crate::static_state::push(&mut *frames, &mut state, 1, 0) || crate::static_state::local_push(&mut *locals, &mut state, 1) { return false; }
    while i > 0 {
        if !crate::static_state::resume(&mut *frames, &state, sequence(i % 16, 9)) || !crate::static_state::resume(&mut *frames, &state, loop_flow(i % 16)) { return false; }
        if frames[i - 1] != i * 16 + loop_flow(i % 16) || crate::static_state::pop(&mut *frames, &mut state) != i || crate::static_state::local_pop(&mut *locals, &mut state) != i { return false; }
        i = i - 1;
    }
    if crate::static_state::pop(&mut *frames, &mut state) != 0 || crate::static_state::local_pop(&mut *locals, &mut state) != 0 || crate::static_state::resume(&mut *frames, &state, 1) { return false; }
    while i < 129 { if frames[i] != 0 || locals[i] != 0 { return false; } i = i + 1; }
    i = 0;
    while i < rows {
        let kind = headers[i] % 64;
        if kind == 1 || kind == 19 || kind == 20 {
            let mut name = headers[i];
            if kind != 1 { name = tokens[ab[i] % 256 - 1]; }
            resolved[i] = crate::static_common::find_function(&*codes, &*headers, name, rows, used);
            semantic[i] = 16;
        }
        if kind == 15 {
            let valid = crate::static_common::decimal(&*codes, tokens[ab[i] % 256 - 1], ab[i] / 256, used, &mut state);
            resolved[i] = state.value;
            semantic[i] = 18;
            if valid { semantic[i] = 17; }
        }
        i = i + 1;
    }
    return resolved[128] == 0 && semantic[128] == 0;
}
