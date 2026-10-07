// One explicit frame per active block. A frame stores block, statement, and
// continuation phase; it never aliases the independent expression frame bank.
fn enter(tokens: &[i32], headers: &[i32], ab: &[i32], seen: &mut [i32], state: &mut crate::ast_validate::State, id: i32) -> i32 {
    if !crate::ast_validate::claim(&*headers, &mut *seen, &mut *state, id, 5, 5, false) || !crate::ast_validate::allocate(&mut *state, id) { return 0; }
    let first = crate::ast_validate::take(&*tokens, &mut *state, 24, 0);
    crate::ast_validate::span(&mut *state, headers[id - 1], first, headers[id - 1]);
    return id * 65536 + ab[id - 1] / 256 * 256;
}
pub fn run(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], heights: &mut [i32], frames: &mut [i32], blocks: &mut [i32], state: &mut crate::ast_validate::State, root: i32) -> () {
    blocks[0] = enter(&*tokens, &*headers, &*ab, &mut *seen, &mut *state, root);
    let mut stack = 1;
    while stack != 0 && state.error == 0 {
        let frame = blocks[stack - 1];
        let block = frame / 65536; let statement = frame % 65536 / 256;
        if statement == 0 {
            let last = crate::ast_validate::take(&*tokens, &mut *state, 25, ab[block - 1] % 256);
            crate::ast_validate::span(&mut *state, headers[block - 1], headers[block - 1], last);
            stack = stack - 1; blocks[stack] = 0;
        } else {
            let mut child = 0;
            if frame % 256 == 0 {
                let value = crate::ast_statement::head(&*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut *state, statement);
                if value != 0 && state.error == 0 {
                    crate::ast_expression::run(&*codes, &*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut *heights, &mut *frames, &mut *state, value);
                }
                if headers[statement - 1] % 64 >= 13 {
                    child = ab[statement - 1] / 256;
                    blocks[stack - 1] = frame + 1;
                } else {
                    crate::ast_statement::finish(&*tokens, &*headers, &*ab, &mut *state, statement);
                }
            } else {
                if frame % 256 == 1 && cd[statement - 1] % 256 != 0 {
                    crate::ast_validate::take(&*tokens, &mut *state, 19, 0);
                    child = cd[statement - 1] % 256;
                    blocks[stack - 1] = frame + 1;
                } else {
                    let mut last = ab[statement - 1] / 256;
                    if frame % 256 == 2 { last = cd[statement - 1] % 256; }
                    crate::ast_validate::span(&mut *state, headers[statement - 1], headers[statement - 1], headers[last - 1]);
                }
            }
            if state.error != 0 { return; }
            if child != 0 {
                if stack >= 64 { state.error = 64; return; }
                blocks[stack] = enter(&*tokens, &*headers, &*ab, &mut *seen, &mut *state, child);
                stack = stack + 1;
            } else {
                blocks[stack - 1] = block * 65536 + headers[statement - 1] / 4194304 * 256;
            }
        }
    }
    return;
}
