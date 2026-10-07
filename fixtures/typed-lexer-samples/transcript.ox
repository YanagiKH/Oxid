use std::io::write_stdout;
use std::io::WriteStatus;

fn write(bytes: &[i32]) -> i32 {
    let result = write_stdout(&*bytes);
    match result {
        WriteStatus::Complete => { return 0; },
        WriteStatus::InvalidInput => { return 70; },
        WriteStatus::IoError(accepted) => { return 74; },
    }
}

// Complete tape preflight precedes every output attempt. Diagnostic tapes are
// empty; unused rows must be zero, including all rows of a diagnostic tape.
fn valid(tokens: &crate::tape::Tape, tag: i32, start: i32, end: i32) -> bool {
    if tag < 0 || tag > 2 { return false; }
    if start < 0 || end < start || end > 128 { return false; }
    if tokens.count < 0 || tokens.count > 129 { return false; }
    if tag == 0 {
        if tokens.count < 1 || start != 0 || end != 0 { return false; }
    } else {
        if tokens.count != 0 || start == end { return false; }
    }
    let mut i = 0;
    let mut prior = 0;
    while i < 129 {
        let kind = tokens.kind[i];
        let lo = tokens.start[i];
        let hi = tokens.end[i];
        if i < tokens.count {
            if lo != prior || hi < lo || hi > 128 { return false; }
            if i == tokens.count - 1 {
                if kind != 47 || lo != hi { return false; }
            } else {
                if kind < 1 || kind > 46 || lo == hi { return false; }
            }
            prior = hi;
        } else {
            if kind != 0 || lo != 0 || hi != 0 { return false; }
        }
        i = i + 1;
    }
    return true;
}

pub fn emit(tokens: &crate::tape::Tape, tag: i32, start: i32, end: i32) -> i32 {
    if !valid(&*tokens, tag, start, end) { return 70; }
    let header = [79, 88, 76, 49, tag, tokens.count, start, end];
    let first = write(&header);
    if first != 0 { return first; }
    let kinds = write(&*tokens.kind);
    if kinds != 0 { return kinds; }
    let starts = write(&*tokens.start);
    if starts != 0 { return starts; }
    return write(&*tokens.end);
}
