mod tape;
mod transcript;
use std::io::read_stdin;
use std::io::ReadStatus;

fn fill_and_check(codes: &[i32], used: i32) -> i32 {
    let mut tokens = crate::tape::new_tape();
    let mut i = 0;
    while i < used {
        tokens.kind[i] = 1;
        tokens.start[i] = i;
        tokens.end[i] = i + 1;
        i = i + 1;
    }
    tokens.kind[used] = 47;
    tokens.start[used] = used;
    tokens.end[used] = used;
    tokens.count = used + 1;
    let mut total = 0;
    i = 0;
    while i < tokens.count {
        total = total + tokens.end[i] - tokens.start[i];
        i = i + 1;
    }
    if total != used { return 70; }
    return crate::transcript::emit(&tokens, 0, 0, 0);
}

fn main() -> i32 {
    let mut codes = crate::tape::zeros();
    let result = read_stdin(&mut codes);
    match result {
        ReadStatus::Eof(used) => {
            if used > 128 { return 64; }
            return fill_and_check(&codes, used);
        },
        ReadStatus::Full => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
