// Experimental token handles contain kind + 64 * physical token slot.
mod buffers; mod source_buffer; mod lexer_core; mod keywords;
mod parser_state; mod parser_signature; mod parser_atom; mod parser_call; mod parser_expression; mod parser_statement; mod parser_control; mod parser_driver; mod parser_output;
use std::io::read_stdin;
use std::io::ReadStatus;
fn pack(kinds: &mut [i32], count: i32) -> () {
    let mut i = 0;
    while i < count { kinds[i] = kinds[i] + 64 * i; i = i + 1; }
    return;
}
fn build(token_starts: &[i32], token_ends: &[i32], codes: &[i32], tokens: &[i32], count: i32, used: i32) -> i32 {
    let mut row_starts = crate::buffers::zeros();
    let mut row_ends = crate::buffers::zeros();
    let mut headers = crate::buffers::zeros();
    let mut ab = crate::buffers::zeros();
    let mut cd = crate::buffers::zeros();
    let mut frames = crate::buffers::zeros();
    let mut state = crate::parser_state::State { cursor: 0, limit: count, used: used, rows: 0, stack: 0, items: 0, tail: 0, mode: 0, value: 0, depth: 0, context: 0, floor: 0, error: 0, detail: 0, start: 0, end: 0 };
    crate::parser_driver::parse(&*token_starts, &*token_ends, &mut row_starts, &mut row_ends, &*codes, &*tokens, &mut headers, &mut ab, &mut cd, &mut frames, &mut state);
    return crate::parser_output::emit(&row_starts, &row_ends, &headers, &ab, &cd, &state);
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
            pack(&mut kinds, count);
            return build(&starts, &ends, &*codes, &kinds, count, used);
        },
        crate::lexer_core::ScanResult::Capacity => { return 64; },
        crate::lexer_core::ScanResult::UnterminatedString(start) => { return crate::parser_output::lexical(used, start, 1); },
        crate::lexer_core::ScanResult::UnterminatedComment(start) => { return crate::parser_output::lexical(used, start, 2); },
    }
}
fn main() -> i32 {
    let mut codes = crate::source_buffer::zeros();
    let result = read_stdin(&mut codes);
    match result {
        ReadStatus::Eof(used) => { return observe(&codes, used); },
        ReadStatus::Full => {
            // 255 source bytes fit the existing native scalar-slot limit.
            // Full is not EOF: consume exactly one additional byte as a probe.
            let mut probe = [0];
            let tail = read_stdin(&mut probe);
            match tail {
                ReadStatus::Eof(count) => { if count == 0 { return observe(&codes, 255); } return 64; },
                ReadStatus::Full => { return 64; },
                ReadStatus::IoError => { return 74; },
            }
        },
        ReadStatus::IoError => { return 74; },
    }
}
