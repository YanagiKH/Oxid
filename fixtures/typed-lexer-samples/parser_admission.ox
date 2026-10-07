mod tape; mod lexer; mod keywords; mod parser_banks; mod parser_probe_output;
use std::io::read_stdin;
use std::io::ReadStatus;

fn probe() -> i32 {
    let mut rows = crate::parser_banks::Rows { header_links: crate::tape::zeros(), ab: crate::tape::zeros(), cd: crate::tape::zeros(), count: 0 };
    let mut stack = crate::parser_banks::Frames { slot: crate::tape::zeros(), count: 0 };
    if crate::parser_banks::pack_header(63, 128, 128, 128) != 538976319 { return 88; }
    if crate::parser_banks::pack_header(0, 0, 0, 0) != -1 { return 89; }
    if crate::parser_banks::pack_header(1, 2, 1, 0) != -1 { return 90; }
    if crate::parser_banks::pack_pair(128, 128) != 32896 { return 93; }
    if crate::parser_banks::pack_pair(129, 0) != -1 { return 94; }
    crate::parser_banks::fill(&mut rows);
    if !crate::parser_banks::valid(&rows) { return 71; }
    rows.header_links[128] = 9;
    if crate::parser_banks::valid(&rows) { return 72; }
    rows.header_links[128] = 0;
    if !crate::parser_banks::seed(&mut stack) { return 73; }
    if crate::parser_banks::seed(&mut stack) { return 75; }
    if crate::parser_banks::push(&mut stack, 2, 1, 65) { return 91; }
    if stack.count != 1 { return 92; }
    let mut i = 1;
    while i <= 128 {
        if !crate::parser_banks::push(&mut stack, 2 + (i - 1) % 20, i, 1984) { return 76; }
        if stack.count != i + 1 { return 77; }
        if crate::parser_banks::push(&mut stack, 2, i, 0) { return 78; }
        if stack.count != i + 1 { return 79; }
        if !crate::parser_banks::resume(&mut stack, 21, 0) { return 80; }
        if stack.slot[i] / 65536 != i || stack.slot[i] % 65536 != 21 { return 81; }
        i = i + 1;
    }
    if stack.count != 129 { return 82; }
    if crate::parser_banks::push(&mut stack, 2, 1, 0) { return 83; }
    i = 128;
    while i > 0 {
        if crate::parser_banks::pop(&mut stack) != i { return 84; }
        if stack.slot[i] != 0 { return 85; }
        i = i - 1;
    }
    if crate::parser_banks::pop(&mut stack) != -1 || stack.count != 1 { return 86; }
    return crate::parser_probe_output::emit(&rows);
}

fn observe(codes: &[i32], used: i32) -> i32 {
    let mut i = 0;
    while i < used { if codes[i] > 127 { return 64; } i = i + 1; }
    let mut tokens = crate::tape::new_tape();
    let scanned = crate::lexer::scan(&*codes, used, &mut tokens);
    match scanned {
        crate::lexer::LexResult::Complete => {
            if tokens.count < 1 || tokens.end[tokens.count - 1] != used { return 87; }
            return probe();
        },
        crate::lexer::LexResult::UnterminatedString(start) => { return 65; },
        crate::lexer::LexResult::UnterminatedComment(start) => { return 65; },
    }
}
fn main() -> i32 {
    let mut codes = crate::tape::zeros();
    let input = read_stdin(&mut codes);
    match input {
        ReadStatus::Eof(used) => { return observe(&codes, used); },
        ReadStatus::Full => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
