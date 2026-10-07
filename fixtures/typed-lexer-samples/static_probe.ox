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
pub fn same_name(codes: &[i32], left: i32, right: i32, used: i32) -> bool {
    let a = crate::parser_state::lo(left);
    let b = crate::parser_state::lo(right);
    let end = crate::parser_state::hi(left);
    let other = crate::parser_state::hi(right);
    if a >= end || b >= other || end > used || other > used || end - a != other - b { return false; }
    let mut i = 0;
    while i < end - a { if codes[a + i] != codes[b + i] { return false; } i = i + 1; }
    return true;
}
pub fn find_function(codes: &[i32], headers: &[i32], name: i32, rows: i32, used: i32) -> i32 {
    let mut i = 0;
    while i < rows {
        if headers[i] % 64 == 1 && same_name(&*codes, headers[i], name, used) { return i + 1; }
        i = i + 1;
    }
    return 0;
}
pub fn decimal(codes: &[i32], token: i32, negative: i32, used: i32, state: &mut crate::static_state::State) -> bool {
    let mut i = crate::parser_state::lo(token);
    let end = crate::parser_state::hi(token);
    state.value = 0;
    if i >= end || end > used || negative < 0 || negative > 1 { return false; }
    let mut value = 0;
    let limit = 7 + negative;
    while i < end {
        let digit = codes[i] - 48;
        if digit < 0 || digit > 9 || value < -214748364 || (value == -214748364 && digit > limit) { return false; }
        value = value * 10 - digit;
        i = i + 1;
    }
    if negative == 0 { value = -value; }
    state.value = value;
    return true;
}
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
            resolved[i] = find_function(&*codes, &*headers, name, rows, used);
            semantic[i] = 16;
        }
        if kind == 15 {
            let valid = decimal(&*codes, tokens[ab[i] % 256 - 1], ab[i] / 256, used, &mut state);
            resolved[i] = state.value;
            semantic[i] = 18;
            if valid { semantic[i] = 17; }
        }
        i = i + 1;
    }
    return resolved[128] == 0 && semantic[128] == 0;
}
