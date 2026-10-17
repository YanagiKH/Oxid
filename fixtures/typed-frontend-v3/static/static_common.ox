// Shared exact source lookup and checked decimal conversion. No probe state or semantics.
pub fn same_name(codes: &[i32], left: i32, right: i32, used: i32) -> bool {
    let mut a = crate::static_span::lo(left);
    let mut b = crate::static_span::lo(right);
    let end = crate::static_span::hi(left);
    let other = crate::static_span::hi(right);
    if a >= end || b >= other || end > used || other > used || end - a != other - b { return false; }
    while a + 4 <= end {
        if word(&*codes, a) != word(&*codes, b) { return false; }
        a = a + 4; b = b + 4;
    }
    while a < end {
        if crate::source_bytes::at(&*codes, a) != crate::source_bytes::at(&*codes, b) { return false; }
        a = a + 1; b = b + 1;
    }
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
    let mut i = crate::static_span::lo(token);
    let end = crate::static_span::hi(token);
    state.value = 0;
    if i >= end || end > used || negative < 0 || negative > 1 { return false; }
    let mut value = 0;
    let limit = 7 + negative;
    while i < end {
        let digit = crate::source_bytes::at(&*codes, i) - 48;
        if digit < 0 || digit > 9 || value < -214748364 || (value == -214748364 && digit > limit) { return false; }
        value = value * 10 - digit;
        i = i + 1;
    }
    if negative == 0 { value = -value; }
    state.value = value;
    return true;
}

// A full four-byte span ends at or before used<=511, so offset<=507.
// Therefore both bank indexes are valid, even when factor1 masks the second.
fn word(codes: &[i32], offset: i32) -> i32 {
    let factor = crate::source_bytes::factor(offset);
    return codes[offset / 4] / factor + (codes[offset / 4 + 1] % factor) * (268435456 / factor);
}
