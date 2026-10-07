// Shared exact source lookup and checked decimal conversion. No probe state or semantics.
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
