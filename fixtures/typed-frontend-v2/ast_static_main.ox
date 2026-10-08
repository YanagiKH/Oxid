// AST2 static consumer: complete validation, resolution, then scalar typing.
// Results are bounded observations, never compiler owners or provider witnesses.
mod buffers; mod source_buffer; mod lexer_core; mod keywords;
mod static_state; mod static_common; mod resolver_names; mod resolver_driver; mod static_column; mod typed_diagnostic; mod typed_expression; mod typed_statement; mod typed_driver; mod typed_output;
mod parser_state; mod ast_output;
mod ast_input; mod ast_validate; mod ast_expression; mod ast_statement; mod ast_block;
fn pack(tokens: &mut [i32], starts: &mut [i32], ends: &mut [i32], count: i32) -> () {
    let mut i = 0;
    while i < count { tokens[i] = tokens[i] + 64 * (starts[i] + 256 * ends[i]); i = i + 1; }
    // Real phase barrier: lexer spans are now in tokens. End their uses and
    // actually clear both owners before reusing them as seen/height banks.
    crate::ast_validate::clear(&mut *starts);
    crate::ast_validate::clear(&mut *ends);
    return;
}
fn validate(codes: &[i32], tokens: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], seen: &mut [i32], heights: &mut [i32], meta: &crate::ast_input::Header, count: i32) -> i32 {
    if !crate::ast_validate::raw(&*headers, &*ab, &*cd, &*meta, count) { return 64; }
    let mut frames = crate::buffers::zeros();
    // Reuse the retained future resolved owner as the block continuation bank.
    let mut resolved = crate::buffers::zeros();
    let mut state = crate::ast_validate::State { cursor: 0, count: count, rows: meta.rows, next: 1, error: 0 };
    let mut item = meta.items;
    while item != 0 && state.error == 0 {
        crate::ast_validate::signature(&*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut state, item);
        if state.error != 0 { break; }
        crate::ast_block::run(&*codes, &*tokens, &*headers, &*ab, &*cd, &mut *seen, &mut *heights, &mut frames, &mut resolved, &mut state, cd[item - 1] / 256);
        item = headers[item - 1] / 4194304;
    }
    if state.error == 0 {
        crate::ast_validate::take(&*tokens, &mut state, 47, 0);
        if state.cursor != count || state.next != meta.rows + 1 { state.error = 64; }
        let mut i = 0;
        while i < meta.rows { if seen[i] != 1 { state.error = 64; } i = i + 1; }
    }
    // No manufactured parser State/root marker and no static_state::release.
    // Clear the actual validation phase owners on completion and error exits.
    crate::ast_validate::clear(&mut *seen);
    crate::ast_validate::clear(&mut *heights);
    crate::ast_validate::clear(&mut frames);
    crate::ast_validate::clear(&mut resolved);
    // Check the real cleared phase boundary before any semantic access.
    let mut semantic = crate::buffers::zeros();
    let mut locals = crate::buffers::zeros();
    let mut i = 0;
    while i < 129 {
        if resolved[i] != 0 || semantic[i] != 0 || locals[i] != 0 || frames[i] != 0 || seen[i] != 0 || heights[i] != 0 { return 70; }
        i = i + 1;
    }
    if state.error != 0 { return state.error; }
    let output = crate::ast_output::emit(&*headers, &*ab, &*cd, &*meta);
    if output != 0 { return output; }
    let mut resolution = crate::static_state::State { stack: 0, locals: 0, value: 0 };
    if !crate::resolver_driver::run(&*codes, &*tokens, &*headers, &*ab, &*cd, &mut resolved, &mut locals, &mut frames, &mut resolution, meta.rows, meta.used) { return 70; }
    if resolution.stack != 0 {
        let kind = resolution.stack;
        let primary = resolution.locals;
        let secondary = resolution.value;
        let mut label = 0;
        if kind == 3 { label = 1; }
        crate::typed_diagnostic::fail(&mut resolution, kind, primary, secondary, label, 0, 0);
    } else {
        if !crate::typed_driver::run(&*tokens, &*headers, &*ab, &*cd, &resolved, &mut semantic, &mut frames, &mut resolution, meta.rows) { return 70; }
    }
    return crate::typed_output::emit(&resolved, &semantic, &resolution, meta.rows, meta.used);
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
        crate::lexer_core::ScanResult::Capacity => { return 64; },
        crate::lexer_core::ScanResult::UnterminatedString(start) => { return 64; },
        crate::lexer_core::ScanResult::UnterminatedComment(start) => { return 64; },
    }
}
fn main() -> i32 {
    let mut codes = crate::source_buffer::zeros();
    let mut headers = crate::buffers::zeros();
    let mut ab = crate::buffers::zeros();
    let mut cd = crate::buffers::zeros();
    let mut meta = crate::ast_input::Header { used: 0, rows: 0, items: 0 };
    let status = crate::ast_input::read(&mut codes, &mut headers, &mut ab, &mut cd, &mut meta);
    if status != 0 { return status; }
    return observe(&codes, &headers, &ab, &cd, &meta);
}
