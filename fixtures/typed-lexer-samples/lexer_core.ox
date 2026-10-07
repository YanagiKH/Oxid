// This scanner accepts only a prevalidated ASCII prefix of at most 128 bytes
// and three fresh zero columns. Kind IDs follow src/frontend/lexer.rs declaration order.
pub enum ScanResult { Complete(i32), UnterminatedString(i32), UnterminatedComment(i32) }
fn whitespace(code: i32) -> bool {
    return code == 32 || (code >= 9 && code <= 13);
}
fn letter(code: i32) -> bool {
    return (code >= 65 && code <= 90) || (code >= 97 && code <= 122) || code == 95;
}
fn digit(code: i32) -> bool {
    return code >= 48 && code <= 57;
}
fn whitespace_end(codes: &[i32], used: i32, from: i32) -> i32 {
    let mut cursor = from;
    while cursor < used {
        if !whitespace(codes[cursor]) { break; }
        cursor = cursor + 1;
    }
    return cursor;
}
fn identifier_end(codes: &[i32], used: i32, from: i32) -> i32 {
    let mut cursor = from;
    while cursor < used {
        let next = codes[cursor];
        if !letter(next) && !digit(next) { break; }
        cursor = cursor + 1;
    }
    return cursor;
}
fn number_end(codes: &[i32], used: i32, from: i32) -> i32 {
    let mut cursor = from;
    while cursor < used {
        let next = codes[cursor];
        if !letter(next) && !digit(next) && next != 46 { break; }
        cursor = cursor + 1;
    }
    return cursor;
}
fn string_end(codes: &[i32], used: i32, from: i32) -> i32 {
    let mut cursor = from;
    while cursor < used {
        let next = codes[cursor];
        cursor = cursor + 1;
        if next == 34 { return cursor; }
        if next == 92 && cursor < used { cursor = cursor + 1; }
    }
    return -1;
}
fn line_comment_end(codes: &[i32], used: i32, from: i32) -> i32 {
    let mut cursor = from;
    while cursor < used {
        if codes[cursor] == 10 { break; }
        cursor = cursor + 1;
    }
    return cursor;
}
fn block_comment_end(codes: &[i32], used: i32, from: i32) -> i32 {
    let mut cursor = from;
    while cursor + 1 < used {
        if codes[cursor] == 42 && codes[cursor + 1] == 47 { return cursor + 2; }
        cursor = cursor + 1;
    }
    return -1;
}
fn single_kind(code: i32) -> i32 {
    if code == 40 { return 22; }
    if code == 41 { return 23; }
    if code == 123 { return 24; }
    if code == 125 { return 25; }
    if code == 58 { return 26; }
    if code == 44 { return 27; }
    if code == 59 { return 28; }
    if code == 61 { return 29; }
    if code == 33 { return 32; }
    if code == 38 { return 10; }
    if code == 46 { return 11; }
    if code == 60 { return 35; }
    if code == 62 { return 37; }
    if code == 45 { return 40; }
    if code == 43 { return 41; }
    if code == 42 { return 42; }
    if code == 47 { return 43; }
    if code == 37 { return 44; }
    if code == 124 || code == 91 || code == 93 || code == 35 || code == 39 { return 45; }
    return 46;
}
// Zero means the next byte does not form one of the seven two-byte operators.
fn pair_kind(code: i32, next: i32) -> i32 {
    if code == 61 && next == 61 { return 30; }
    if code == 33 && next == 61 { return 31; }
    if code == 38 && next == 38 { return 33; }
    if code == 124 && next == 124 { return 34; }
    if code == 60 && next == 61 { return 36; }
    if code == 62 && next == 61 { return 38; }
    if code == 45 && next == 62 { return 39; }
    return 0;
}
fn clear(kinds: &mut [i32], starts: &mut [i32], ends: &mut [i32]) -> () {
    let mut i = 0;
    while i < 129 {
        kinds[i] = 0;
        starts[i] = 0;
        ends[i] = 0;
        i = i + 1;
    }
    return;
}
pub fn scan(codes: &[i32], used: i32, kinds: &mut [i32], starts: &mut [i32], ends: &mut [i32]) -> ScanResult {
    let mut count = 0;
    let mut cursor = 0;
    while cursor < used {
        let start = cursor;
        let code = codes[cursor];
        cursor = cursor + 1;
        let mut next = -1;
        if cursor < used { next = codes[cursor]; }
        let mut kind = 0;
        if whitespace(code) {
            kind = 1;
            cursor = whitespace_end(&*codes, used, cursor);
        } else {
            if code == 47 && next == 47 {
                kind = 1;
                cursor = line_comment_end(&*codes, used, cursor);
            } else {
                if code == 47 && next == 42 {
                    kind = 1;
                    cursor = block_comment_end(&*codes, used, cursor + 1);
                    if cursor < 0 {
                        clear(&mut *kinds, &mut *starts, &mut *ends);
                        return ScanResult::UnterminatedComment(start);
                    }
                } else {
                    if letter(code) {
                        cursor = identifier_end(&*codes, used, cursor);
                        kind = crate::keywords::classify(&*codes, start, cursor);
                    } else {
                        if digit(code) {
                            kind = 3;
                            cursor = number_end(&*codes, used, cursor);
                        } else {
                            if code == 34 {
                                kind = 4;
                                cursor = string_end(&*codes, used, cursor);
                                if cursor < 0 {
                                    clear(&mut *kinds, &mut *starts, &mut *ends);
                                    return ScanResult::UnterminatedString(start);
                                }
                            } else {
                                kind = pair_kind(code, next);
                                if kind == 0 {
                                    kind = single_kind(code);
                                } else {
                                    cursor = cursor + 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        // Every non-EOF token consumes at least one input byte, so at most
        // 128 rows precede EOF in the 129-row tape.
        kinds[count] = kind;
        starts[count] = start;
        ends[count] = cursor;
        count = count + 1;
    }
    kinds[count] = 47;
    starts[count] = used;
    ends[count] = used;
    count = count + 1;
    return ScanResult::Complete(count);
}
