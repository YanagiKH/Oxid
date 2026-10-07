// Called only after every child expression has completed, in canonical postorder.
pub fn finish(headers: &[i32], ab: &[i32], resolved: &[i32], semantic: &mut [i32], state: &mut crate::static_state::State, node: i32) -> bool {
    let kind = headers[node - 1] % 64;
    let a = ab[node - 1] % 256;
    let mut result = 1;
    if kind == 15 { result = 2; }
    if kind == 18 { result = 3; }
    if kind == 19 { result = semantic[resolved[node - 1] - 1]; }
    if kind >= 21 {
        result = semantic[a - 1];
        if kind != 21 {
            let mut expected = 2;
            if kind == 23 || kind >= 35 { expected = 1; }
            if kind == 29 || kind == 30 {
                expected = result;
                if expected == 3 { return crate::typed_diagnostic::fail(&mut *state, 9, headers[a - 1], 0, 0, 0, 0); }
            }
            if !crate::typed_diagnostic::expect(&mut *state, expected, result, headers[a - 1], 0, 0) { return false; }
            if kind >= 24 {
                let b = ab[node - 1] / 256;
                if !crate::typed_diagnostic::expect(&mut *state, expected, semantic[b - 1], headers[b - 1], 0, 0) { return false; }
            }
            result = expected;
            if kind >= 29 { result = 1; }
        }
    }
    semantic[node - 1] = result;
    return true;
}
pub fn call(headers: &[i32], ab: &[i32], resolved: &[i32], semantic: &mut [i32], state: &mut crate::static_state::State, node: i32) -> bool {
    let target = resolved[node - 1];
    let mut parameter = ab[target - 1] / 256;
    let mut argument = ab[node - 1] / 256;
    let mut expected = 0;
    let mut actual = 0;
    while parameter != 0 { expected = expected + 1; parameter = headers[parameter - 1] / 4194304; }
    while argument != 0 { actual = actual + 1; argument = headers[argument - 1] / 4194304; }
    if expected != actual { return crate::typed_diagnostic::fail(&mut *state, 10, headers[node - 1], headers[target - 1], 2, expected, actual); }
    parameter = ab[target - 1] / 256; argument = ab[node - 1] / 256;
    while argument != 0 {
        if !crate::typed_diagnostic::expect(&mut *state, semantic[parameter - 1], semantic[argument - 1], headers[argument - 1], headers[target - 1], 2) { return false; }
        parameter = headers[parameter - 1] / 4194304;
        argument = headers[argument - 1] / 4194304;
    }
    semantic[node - 1] = semantic[target - 1];
    return true;
}
