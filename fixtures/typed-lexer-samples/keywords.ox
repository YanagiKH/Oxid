// Exact ASCII keyword spellings from src/frontend/lexer.rs.
// Length dispatch prevents prefix matches and keeps every comparison in bounds.
pub fn classify(codes: &[i32], start: i32, end: i32) -> i32 {
    let size = end - start;
    if size == 2 { return length_2(&*codes, start); }
    if size == 3 { return length_3(&*codes, start); }
    if size == 4 { return length_4(&*codes, start); }
    if size == 5 { return length_5(&*codes, start); }
    if size == 6 { return length_6(&*codes, start); }
    if size == 8 { return length_8(&*codes, start); }
    if size == 11 { return length_11(&*codes, start); }
    return 2;
}

fn length_2(codes: &[i32], start: i32) -> i32 {
    // fn
    if codes[start] == 102 && codes[start + 1] == 110 { return 5; }
    // if
    if codes[start] == 105 && codes[start + 1] == 102 { return 17; }
    // or
    if codes[start] == 111 && codes[start + 1] == 114 { return 45; }
    return 2;
}

fn length_3(codes: &[i32], start: i32) -> i32 {
    // mod
    if codes[start] == 109 && codes[start + 1] == 111 && codes[start + 2] == 100 { return 7; }
    // use
    if codes[start] == 117 && codes[start + 1] == 115 && codes[start + 2] == 101 { return 8; }
    // pub
    if codes[start] == 112 && codes[start + 1] == 117 && codes[start + 2] == 98 { return 9; }
    // let
    if codes[start] == 108 && codes[start + 1] == 101 && codes[start + 2] == 116 { return 12; }
    // mut
    if codes[start] == 109 && codes[start + 1] == 117 && codes[start + 2] == 116 { return 13; }
    // for
    if codes[start] == 102 && codes[start + 1] == 111 && codes[start + 2] == 114 { return 45; }
    // ref
    if codes[start] == 114 && codes[start + 1] == 101 && codes[start + 2] == 102 { return 45; }
    // and
    if codes[start] == 97 && codes[start + 1] == 110 && codes[start + 2] == 100 { return 45; }
    return 2;
}

fn length_4(codes: &[i32], start: i32) -> i32 {
    // else
    if codes[start] == 101 && codes[start + 1] == 108 && codes[start + 2] == 115 && codes[start + 3] == 101 { return 19; }
    // true
    if codes[start] == 116 && codes[start + 1] == 114 && codes[start + 2] == 117 && codes[start + 3] == 101 { return 20; }
    // loop
    if codes[start] == 108 && codes[start + 1] == 111 && codes[start + 2] == 111 && codes[start + 3] == 112 { return 45; }
    // move
    if codes[start] == 109 && codes[start + 1] == 111 && codes[start + 2] == 118 && codes[start + 3] == 101 { return 45; }
    // enum
    if codes[start] == 101 && codes[start + 1] == 110 && codes[start + 2] == 117 && codes[start + 3] == 109 { return 45; }
    // impl
    if codes[start] == 105 && codes[start + 1] == 109 && codes[start + 2] == 112 && codes[start + 3] == 108 { return 45; }
    // type
    if codes[start] == 116 && codes[start + 1] == 121 && codes[start + 2] == 112 && codes[start + 3] == 101 { return 45; }
    // null
    if codes[start] == 110 && codes[start + 1] == 117 && codes[start + 2] == 108 && codes[start + 3] == 108 { return 45; }
    return 2;
}

fn length_5(codes: &[i32], start: i32) -> i32 {
    // break
    if codes[start] == 98 && codes[start + 1] == 114 && codes[start + 2] == 101 && codes[start + 3] == 97 && codes[start + 4] == 107 { return 15; }
    // while
    if codes[start] == 119 && codes[start + 1] == 104 && codes[start + 2] == 105 && codes[start + 3] == 108 && codes[start + 4] == 101 { return 18; }
    // false
    if codes[start] == 102 && codes[start + 1] == 97 && codes[start + 2] == 108 && codes[start + 3] == 115 && codes[start + 4] == 101 { return 21; }
    return unsupported_5(&*codes, start);
}

// A separate helper keeps the native per-function scalar-slot ceiling intact.
fn unsupported_5(codes: &[i32], start: i32) -> i32 {
    // macro
    if codes[start] == 109 && codes[start + 1] == 97 && codes[start + 2] == 99 && codes[start + 3] == 114 && codes[start + 4] == 111 { return 45; }
    // const
    if codes[start] == 99 && codes[start + 1] == 111 && codes[start + 2] == 110 && codes[start + 3] == 115 && codes[start + 4] == 116 { return 45; }
    // match
    if codes[start] == 109 && codes[start + 1] == 97 && codes[start + 2] == 116 && codes[start + 3] == 99 && codes[start + 4] == 104 { return 45; }
    // async
    if codes[start] == 97 && codes[start + 1] == 115 && codes[start + 2] == 121 && codes[start + 3] == 110 && codes[start + 4] == 99 { return 45; }
    // await
    if codes[start] == 97 && codes[start + 1] == 119 && codes[start + 2] == 97 && codes[start + 3] == 105 && codes[start + 4] == 116 { return 45; }
    // trait
    if codes[start] == 116 && codes[start + 1] == 114 && codes[start + 2] == 97 && codes[start + 3] == 105 && codes[start + 4] == 116 { return 45; }
    return 2;
}

fn length_6(codes: &[i32], start: i32) -> i32 {
    // struct
    if codes[start] == 115 && codes[start + 1] == 116 && codes[start + 2] == 114 && codes[start + 3] == 117 && codes[start + 4] == 99 && codes[start + 5] == 116 { return 6; }
    // return
    if codes[start] == 114 && codes[start + 1] == 101 && codes[start + 2] == 116 && codes[start + 3] == 117 && codes[start + 4] == 114 && codes[start + 5] == 110 { return 14; }
    // import
    if codes[start] == 105 && codes[start + 1] == 109 && codes[start + 2] == 112 && codes[start + 3] == 111 && codes[start + 4] == 114 && codes[start + 5] == 116 { return 45; }
    // unsafe
    if codes[start] == 117 && codes[start + 1] == 110 && codes[start + 2] == 115 && codes[start + 3] == 97 && codes[start + 4] == 102 && codes[start + 5] == 101 { return 45; }
    // extern
    if codes[start] == 101 && codes[start + 1] == 120 && codes[start + 2] == 116 && codes[start + 3] == 101 && codes[start + 4] == 114 && codes[start + 5] == 110 { return 45; }
    return 2;
}

fn length_8(codes: &[i32], start: i32) -> i32 {
    // continue
    if codes[start] == 99 && codes[start + 1] == 111 && codes[start + 2] == 110 && codes[start + 3] == 116 && codes[start + 4] == 105 && codes[start + 5] == 110 && codes[start + 6] == 117 && codes[start + 7] == 101 { return 16; }
    return 2;
}

fn length_11(codes: &[i32], start: i32) -> i32 {
    // macro_rules
    if codes[start] == 109 && codes[start + 1] == 97 && codes[start + 2] == 99 && codes[start + 3] == 114 && codes[start + 4] == 111 && codes[start + 5] == 95 && codes[start + 6] == 114 && codes[start + 7] == 117 && codes[start + 8] == 108 && codes[start + 9] == 101 && codes[start + 10] == 115 { return 45; }
    return 2;
}
