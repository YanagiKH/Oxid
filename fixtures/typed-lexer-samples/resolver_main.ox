// Actual bounded scalar resolution. Successful STF1 tag 2 is pending typing.
mod buffers; mod lexer_core; mod keywords;
mod static_state; mod static_common; mod resolver_names; mod resolver_driver; mod resolver_output; mod static_output;
mod parser_state; mod parser_signature; mod parser_atom; mod parser_call; mod parser_expression; mod parser_statement; mod parser_control; mod parser_driver; mod parser_output;
use std::io::read_stdin;
use std::io::ReadStatus;
fn pack(kinds: &mut [i32], starts: &[i32], ends: &[i32], count: i32) -> () {
    let mut i = 0;
    while i < count { kinds[i] = kinds[i] + 64 * (starts[i] + 256 * ends[i]); i = i + 1; }
    return;
}
fn build(codes: &[i32], tokens: &[i32], count: i32, used: i32) -> i32 {
    let mut headers = crate::buffers::zeros();
    let mut ab = crate::buffers::zeros();
    let mut cd = crate::buffers::zeros();
    let mut frames = crate::buffers::zeros();
    let mut state = crate::parser_state::State { cursor: 0, limit: count, used: used, rows: 0, stack: 0, items: 0, tail: 0, mode: 0, value: 0, depth: 0, context: 0, floor: 0, error: 0, detail: 0, start: 0, end: 0 };
    crate::parser_driver::parse(&*codes, &*tokens, &mut headers, &mut ab, &mut cd, &mut frames, &mut state);
    let syntax = crate::parser_output::emit(&headers, &ab, &cd, &state);
    if syntax != 0 || state.error != 0 { return syntax; }
    if !crate::static_state::release(&mut frames, &state) { return 70; }
    let mut resolved = crate::buffers::zeros();
    let mut semantic = crate::buffers::zeros();
    let mut locals = crate::buffers::zeros();
    let mut resolution = crate::static_state::State { stack: 0, locals: 0, value: 0 };
    if !crate::resolver_driver::run(&*codes, &*tokens, &headers, &ab, &cd, &mut resolved, &mut locals, &mut frames, &mut resolution, state.rows, used) { return 70; }
    return crate::resolver_output::emit(&resolved, &semantic, &resolution, state.rows, used);
}
fn observe(codes: &[i32], used: i32) -> i32 {
    let mut i = 0;
    while i < used { if codes[i] > 127 { return 64; } i = i + 1; }
    let mut kinds = crate::buffers::zeros();
    let mut starts = crate::buffers::zeros();
    let mut ends = crate::buffers::zeros();
    let result = crate::lexer_core::scan(&*codes, used, &mut kinds, &mut starts, &mut ends);
    match result {
        crate::lexer_core::ScanResult::Complete(count) => {
            pack(&mut kinds, &starts, &ends, count);
            return build(&*codes, &kinds, count, used);
        },
        crate::lexer_core::ScanResult::UnterminatedString(start) => { return crate::parser_output::lexical(used, start, 1); },
        crate::lexer_core::ScanResult::UnterminatedComment(start) => { return crate::parser_output::lexical(used, start, 2); },
    }
}
fn main() -> i32 {
    let mut codes = crate::buffers::zeros();
    let result = read_stdin(&mut codes);
    match result {
        ReadStatus::Eof(used) => { return observe(&codes, used); },
        ReadStatus::Full => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
