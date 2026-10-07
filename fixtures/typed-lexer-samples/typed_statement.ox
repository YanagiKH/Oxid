// RHS/initializer/value children have already completed before these constraints.
pub fn finish(tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], resolved: &[i32], semantic: &mut [i32], state: &mut crate::static_state::State, node: i32) -> bool {
    let kind = headers[node - 1] % 64;
    if kind >= 6 && kind <= 8 {
        let value = cd[node - 1] % 256;
        let actual = semantic[value - 1];
        let mut declaration = node;
        let mut expected = actual;
        if kind == 8 {
            declaration = resolved[node - 1];
            if headers[declaration - 1] % 64 != 7 {
                return crate::typed_diagnostic::fail(&mut *state, 14, tokens[ab[node - 1] % 256 - 1], crate::resolver_names::name(&*tokens, &*headers, &*ab, declaration), 4, 0, 0);
            }
            expected = semantic[declaration - 1];
        } else {
            let annotation = ab[node - 1] / 256;
            if annotation != 0 { expected = resolved[annotation - 1]; }
        }
        if !crate::typed_diagnostic::expect(&mut *state, expected, actual, headers[value - 1], crate::resolver_names::name(&*tokens, &*headers, &*ab, declaration), 3) { return false; }
        if kind != 8 { semantic[node - 1] = actual; }
    }
    if kind == 10 {
        let value = ab[node - 1] % 256;
        let mut actual = 3;
        let mut primary = headers[node - 1];
        if value != 0 { actual = semantic[value - 1]; primary = headers[value - 1]; }
        let expected = state.locals;
        if !crate::typed_diagnostic::expect(&mut *state, expected, actual, primary, 0, 0) { return false; }
    }
    return true;
}
pub fn union(left: i32, right: i32) -> i32 {
    let mut result = 0;
    let mut bit = 1;
    while bit <= 8 {
        if left / bit % 2 != 0 || right / bit % 2 != 0 { result = result + bit; }
        bit = bit * 2;
    }
    return result;
}
pub fn flow(headers: &[i32], ab: &[i32], cd: &[i32], semantic: &[i32], node: i32) -> i32 {
    let kind = headers[node - 1] % 64;
    if kind == 10 { return 2; }
    if kind == 11 { return 4; }
    if kind == 12 { return 8; }
    if kind == 14 { return 1 + 2 * (semantic[ab[node - 1] / 256 - 1] / 2 % 2); }
    if kind == 13 {
        let other = cd[node - 1] % 256;
        let mut otherwise = 1;
        if other != 0 { otherwise = semantic[other - 1]; }
        return union(semantic[ab[node - 1] / 256 - 1], otherwise);
    }
    return 1;
}
