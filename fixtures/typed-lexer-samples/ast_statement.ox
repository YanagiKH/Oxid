// Statement heads consume syntax only. Name/type meanings and loop placement
// belong to later semantic phases, which remain unreachable in this consumer.
pub fn head(tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], state: &mut crate::ast_validate::State, id: i32) -> i32 {
    if !crate::ast_validate::claim(&*headers, &mut *seen, &mut *state, id, 6, 14, true) { return 0; }
    let h = headers[id - 1]; let kind = h % 64;
    if kind == 9 { return ab[id - 1] % 256; }
    if !crate::ast_validate::allocate(&mut *state, id) { return 0; }
    let mut expected = kind + 4;
    if kind == 6 || kind == 7 { expected = 12; }
    if kind == 8 { expected = 2; }
    let mut reference = 0;
    if kind == 8 { reference = ab[id - 1] % 256; }
    let first = crate::ast_validate::take(&*tokens, &mut *state, expected, reference);
    crate::ast_validate::span(&mut *state, h, first, h);
    if kind == 6 || kind == 7 {
        if kind == 7 { crate::ast_validate::take(&*tokens, &mut *state, 13, 0); }
        crate::ast_validate::take(&*tokens, &mut *state, 2, ab[id - 1] % 256);
        if ab[id - 1] / 256 != 0 {
            crate::ast_validate::take(&*tokens, &mut *state, 26, 0);
            crate::ast_validate::ty(&*tokens, &*headers, &*ab, &mut *seen, &mut *state, ab[id - 1] / 256);
        }
        crate::ast_validate::take(&*tokens, &mut *state, 29, 0);
    }
    if kind == 8 { crate::ast_validate::take(&*tokens, &mut *state, 29, ab[id - 1] / 256); }
    if kind <= 8 { return cd[id - 1] % 256; }
    return ab[id - 1] % 256;
}
pub fn finish(tokens: &[i32], headers: &[i32], ab: &[i32], state: &mut crate::ast_validate::State, id: i32) -> () {
    let h = headers[id - 1];
    let last = crate::ast_validate::take(&*tokens, &mut *state, 28, 0);
    let mut first = h;
    if h % 64 == 9 {
        // ExprStmt is allocated after its whole expression AND semicolon.
        if !crate::ast_validate::allocate(&mut *state, id) { return; }
        first = headers[ab[id - 1] % 256 - 1];
    }
    crate::ast_validate::span(&mut *state, h, first, last);
    return;
}
