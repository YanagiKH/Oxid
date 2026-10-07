mod buffers;
mod tape;
mod transcript;
mod lexer;
mod lexer_core;
mod keywords;
use std::io::read_stdin;
use std::io::ReadStatus;
fn observe(codes: &[i32], used: i32) -> i32 {
    let mut i = 0;
    while i < used {
        if codes[i] < 0 || codes[i] > 127 { return 64; }
        i = i + 1;
    }
    let mut tokens = crate::tape::new_tape();
    let scanned = crate::lexer::scan(&*codes, used, &mut tokens);
    match scanned {
        crate::lexer::LexResult::Complete => {
            return crate::transcript::emit(&tokens, 0, 0, 0);
        },
        crate::lexer::LexResult::UnterminatedString(start) => {
            return crate::transcript::emit(&tokens, 1, start, used);
        },
        crate::lexer::LexResult::UnterminatedComment(start) => {
            return crate::transcript::emit(&tokens, 2, start, used);
        },
    }
}
fn main() -> i32 {
    let mut codes = crate::tape::zeros();
    let result = read_stdin(&mut codes);
    match result {
        ReadStatus::Eof(used) => { return observe(&codes, used); },
        ReadStatus::Full => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
