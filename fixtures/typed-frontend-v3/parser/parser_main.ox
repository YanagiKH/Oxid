// Experimental token handles contain kind + 64 * physical token slot.
mod buffers; mod source_bytes; mod lexer_core; mod keywords;
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
// Only the fresh prefix returned by read_stdin is consumed. Full is not EOF.
fn main() -> i32 {
    let mut codes = crate::source_bytes::zeros();
    let mut scratch = crate::source_bytes::zeros();
    let mut used = 0;
    while used <= 384 {
        let result = read_stdin(&mut scratch);
        let mut count = 0;
        let mut eof = false;
        match result {
            ReadStatus::Eof(n) => { count = n; eof = true; },
            ReadStatus::Full => { count = 128; },
            ReadStatus::IoError => { return 74; },
        }
        if count < 0 || count > 128 || used + count > 511 { return 64; }
        let mut i = 0;
        while i < count {
            let byte = scratch[i];
            if byte < 0 || byte > 127 { return 64; }
            crate::source_bytes::put(&mut codes, used + i, byte);
            i = i + 1;
        }
        used = used + count;
        if eof { return observe(&codes, used); }
    }
    return 64;
}
