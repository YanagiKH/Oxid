// CLOSED AST1 admission experiment. The full semantic modules remain retained
// and normally checked, but semantic entry is absent. Nothing writes stdout.
// Every representative completion and unimplemented family returns 70. This
// executable never grants an accepted AST, TypedProgram, or source witness.
mod buffers; mod lexer_core; mod keywords;
mod static_state; mod static_common; mod resolver_names; mod resolver_driver; mod static_column; mod typed_diagnostic; mod typed_expression; mod typed_statement; mod typed_driver; mod typed_output;
mod parser_state; mod parser_output;
mod ast_input; mod ast_validate; mod ast_expression;
fn pack(tokens: &mut [i32], starts: &mut [i32], ends: &mut [i32], count: i32) -> () {
    let mut i = 0;
    while i < count { tokens[i] = tokens[i] + 64 * (starts[i] + 256 * ends[i]); i = i + 1; }
    // Real phase barrier: lexer spans are now in tokens. End their uses and
    // actually clear both owners before reusing them as seen/height banks.
    crate::ast_validate::clear(&mut *starts);
    crate::ast_validate::clear(&mut *ends);
    return;
}
fn block(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], heights: &mut [i32], frames: &mut [i32], state: &mut crate::ast_validate::State, id: i32) -> () {
    if !crate::ast_validate::claim(&*headers, &mut *seen, &mut *state, id, 5, 5, false) || !crate::ast_validate::allocate(&mut *state, id) { return; }
    let first = crate::ast_validate::take(&*tokens, &mut *state, 24, 0);
    let mut statement = ab[id - 1] / 256;
    while statement != 0 && state.error == 0 {
        if !crate::ast_validate::claim(&*headers, &mut *seen, &mut *state, statement, 6, 14, true) { return; }
        if headers[statement - 1] % 64 != 9 { state.error = 70; return; }
        let value = ab[statement - 1] % 256;
        crate::ast_expression::run(&*codes, &*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut *heights, &mut *frames, &mut *state, value);
        if state.error != 0 { return; }
        let last = crate::ast_validate::take(&*tokens, &mut *state, 28, 0);
        // ExprStmt is allocated only after its expression AND semicolon.
        if !crate::ast_validate::allocate(&mut *state, statement) { return; }
        crate::ast_validate::span(&mut *state, headers[statement - 1], headers[value - 1], last);
        statement = headers[statement - 1] / 4194304;
    }
    let last = crate::ast_validate::take(&*tokens, &mut *state, 25, ab[id - 1] % 256);
    crate::ast_validate::span(&mut *state, headers[id - 1], first, last);
    return;
}
fn validate(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], heights: &mut [i32], meta: &crate::ast_input::Header, count: i32) -> i32 {
    if !crate::ast_validate::raw(&*headers, &*ab, &*cd, &*meta, count) { return 64; }
    let mut frames = crate::buffers::zeros();
    let mut state = crate::ast_validate::State { cursor: 0, count: count, rows: meta.rows, next: 1, error: 0 };
    let mut item = meta.items;
    while item != 0 && state.error == 0 {
        crate::ast_validate::signature(&*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut state, item);
        if state.error != 0 { break; }
        block(&*codes, &*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut *heights, &mut frames, &mut state, cd[item - 1] / 256);
        item = headers[item - 1] / 4194304;
    }
    if state.error == 0 {
        crate::ast_validate::take(&*tokens, &mut state, 47, 0);
        if state.cursor != count || state.next != meta.rows + 1 { state.error = 64; }
        let mut i = 0;
        while i < meta.rows { if seen[i] != 1 { state.error = 64; } i = i + 1; }
    }
    // No manufactured parser State/root marker and no static_state::release.
    // Clear the actual validation phase owners even on pending/error exits.
    crate::ast_validate::clear(&mut *seen);
    crate::ast_validate::clear(&mut *heights);
    crate::ast_validate::clear(&mut frames);
    // Retain and charge maximum future semantic banks with a zero handoff
    // check, while keeping all actual semantic entry calls absent.
    let resolved = crate::buffers::zeros();
    let semantic = crate::buffers::zeros();
    let locals = crate::buffers::zeros();
    let mut i = 0;
    while i < 129 {
        if resolved[i] != 0 || semantic[i] != 0 || locals[i] != 0 || frames[i] != 0 || seen[i] != 0 || heights[i] != 0 { return 70; }
        i = i + 1;
    }
    if state.error != 0 { return state.error; }
    return 70;
}
fn observe(codes: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], meta: &crate::ast_input::Header) -> i32 {
    let mut tokens = crate::buffers::zeros();
    let mut starts = crate::buffers::zeros();
    let mut ends = crate::buffers::zeros();
    let result = crate::lexer_core::scan(&*codes, meta.used, &mut tokens, &mut starts, &mut ends);
    match result {
        crate::lexer_core::ScanResult::Complete(count) => {
            pack(&mut tokens, &mut starts, &mut ends, count);
            return validate(&*codes, &tokens, &*headers, &*ab, &*cd, &mut starts, &mut ends, &*meta, count);
        },
        crate::lexer_core::ScanResult::UnterminatedString(start) => { return 64; },
        crate::lexer_core::ScanResult::UnterminatedComment(start) => { return 64; },
    }
}
fn main() -> i32 {
    let mut codes = crate::buffers::zeros();
    let mut headers = crate::buffers::zeros();
    let mut ab = crate::buffers::zeros();
    let mut cd = crate::buffers::zeros();
    let mut meta = crate::ast_input::Header { used: 0, rows: 0, items: 0 };
    let status = crate::ast_input::read(&mut codes, &mut headers, &mut ab, &mut cd, &mut meta);
    if status != 0 { return status; }
    return observe(&codes, &headers, &ab, &cd, &meta);
}
