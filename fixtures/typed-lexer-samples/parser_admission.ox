mod tape; mod lexer; mod keywords; mod parser_banks; mod parser_probe_output;
use std::io::read_stdin;
use std::io::ReadStatus;

fn probe() -> i32 {
    let mut header = crate::tape::zeros();
    let mut ab = crate::tape::zeros();
    let mut cd = crate::tape::zeros();
    let mut stack = crate::tape::zeros();
    let mut counts = crate::parser_banks::Counts { rows: 0, stack: 0 };
    if crate::parser_banks::pack_header(63, 128, 128, 128) != 538976319 { return 88; }
    if crate::parser_banks::pack_header(0, 0, 0, 0) != -1 { return 89; }
    if crate::parser_banks::pack_header(1, 2, 1, 0) != -1 { return 90; }
    if crate::parser_banks::pack_pair(128, 128) != 32896 { return 93; }
    if crate::parser_banks::pack_pair(129, 0) != -1 { return 94; }
    crate::parser_banks::fill(&mut header, &mut ab, &mut cd, &mut counts);
    if !crate::parser_banks::valid(&header, &ab, &cd, counts.rows) { return 71; }
    header[128] = 9;
    if crate::parser_banks::valid(&header, &ab, &cd, counts.rows) { return 72; }
    header[128] = 0;
    if !crate::parser_banks::seed(&mut stack, &mut counts) { return 73; }
    if crate::parser_banks::seed(&mut stack, &mut counts) { return 75; }
    if crate::parser_banks::push(&mut stack, &mut counts, 2, 1, 65) { return 91; }
    if counts.stack != 1 { return 92; }
    let mut i = 1;
    while i <= 128 {
        if !crate::parser_banks::push(&mut stack, &mut counts, 2 + (i - 1) % 20, i, 1984) { return 76; }
        if counts.stack != i + 1 { return 77; }
        if crate::parser_banks::push(&mut stack, &mut counts, 2, i, 0) { return 78; }
        if counts.stack != i + 1 { return 79; }
        if !crate::parser_banks::resume(&mut stack, &mut counts, 21, 0) { return 80; }
        if stack[i] / 65536 != i || stack[i] % 65536 != 21 { return 81; }
        i = i + 1;
    }
    if counts.stack != 129 { return 82; }
    if crate::parser_banks::push(&mut stack, &mut counts, 2, 1, 0) { return 83; }
    i = 128;
    while i > 0 {
        if crate::parser_banks::pop(&mut stack, &mut counts) != i { return 84; }
        if stack[i] != 0 { return 85; }
        i = i - 1;
    }
    if crate::parser_banks::pop(&mut stack, &mut counts) != -1 || counts.stack != 1 { return 86; }
    return crate::parser_probe_output::emit(&header, &ab, &cd, counts.rows);
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
