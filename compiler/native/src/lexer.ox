use crate::model::Compiler;
use crate::model::Token;
use crate::model::grown;
use crate::model::word;
use crate::model::code;
use alloc::bytes::len as bytes_len;
use alloc::vec::len;
use alloc::vec::try_push;

fn letter(b: i32) -> bool {
    return b == 95 || (b >= 65 && b <= 90) || (b >= 97 && b <= 122);
}
fn digit(b: i32) -> bool { return b >= 48 && b <= 57; }
fn keyword(source: &[u8], start: i32, end: i32) -> i32 {
    let literal_bytes_0 = b"fn";
    let literal_bytes_1 = b"let";
    let literal_bytes_2 = b"mut";
    let literal_bytes_3 = b"return";
    let literal_bytes_4 = b"if";
    let literal_bytes_5 = b"else";
    let literal_bytes_6 = b"while";
    let literal_bytes_7 = b"break";
    let literal_bytes_8 = b"continue";
    let literal_bytes_9 = b"true";
    let literal_bytes_10 = b"false";
    let literal_bytes_11 = b"i32";
    let literal_bytes_12 = b"bool";

    if word(&*source, start, end, &literal_bytes_0) { return 3; }
    if word(&*source, start, end, &literal_bytes_1) { return 4; }
    if word(&*source, start, end, &literal_bytes_2) { return 5; }
    if word(&*source, start, end, &literal_bytes_3) { return 6; }
    if word(&*source, start, end, &literal_bytes_4) { return 7; }
    if word(&*source, start, end, &literal_bytes_5) { return 8; }
    if word(&*source, start, end, &literal_bytes_6) { return 9; }
    if word(&*source, start, end, &literal_bytes_7) { return 10; }
    if word(&*source, start, end, &literal_bytes_8) { return 11; }
    if word(&*source, start, end, &literal_bytes_9) { return 12; }
    if word(&*source, start, end, &literal_bytes_10) { return 13; }
    if word(&*source, start, end, &literal_bytes_11) { return 14; }
    if word(&*source, start, end, &literal_bytes_12) { return 15; }
    return 1;
}
fn push(c: &mut Compiler, kind: i32, start: i32, end: i32, value: i32) -> () {
    if len(&*c.tokens) >= 32768 { c.error = 68; return; }
    if !grown(try_push(&mut *c.tokens, Token { kind: kind, start: start, end: end, value: value })) {
        c.error = 68;
    }
    return;
}
fn block_comment(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let mut i = start + 2;
    let mut depth = 1;
    while i < size && depth > 0 {
        if i + 1 < size && code(&*c.source, i) == 47 && code(&*c.source, i + 1) == 42 {
            depth = depth + 1;
            i = i + 2;
            if depth > 64 { c.error = 68; return i; }
        } else {
            if i + 1 < size && code(&*c.source, i) == 42 && code(&*c.source, i + 1) == 47 {
                depth = depth - 1;
                i = i + 2;
            } else { i = i + 1; }
        }
    }
    if depth != 0 { c.error = 65; c.error_start = start; }
    return i;
}
fn identifier(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let mut i = start + 1;
    while i < size && (letter(code(&*c.source, i)) || digit(code(&*c.source, i))) { i = i + 1; }
    let keyword_kind = keyword(&*c.source, start, i);
    push(&mut *c, keyword_kind, start, i, 0);
    return i;
}
fn integer(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let mut i = start;
    let mut value = 0;
    while i < size && digit(code(&*c.source, i)) {
        let d = code(&*c.source, i) - 48;
        if value < -214748364 || (value == -214748364 && d > 8) {
            c.error = 65; c.error_start = start; return i;
        }
        value = value * 10 - d;
        i = i + 1;
    }
    push(&mut *c, 2, start, i, value);
    return i;
}
fn single_kind(b: i32) -> i32 {
    if b == 40 { return 20; }
    if b == 41 { return 21; }
    if b == 123 { return 22; }
    if b == 125 { return 23; }
    if b == 58 { return 24; }
    if b == 59 { return 25; }
    if b == 44 { return 26; }
    if b == 61 { return 28; }
    if b == 43 { return 29; }
    if b == 45 { return 30; }
    if b == 42 { return 31; }
    if b == 47 { return 32; }
    if b == 37 { return 33; }
    if b == 33 { return 34; }
    if b == 60 { return 37; }
    if b == 62 { return 39; }
    return 0;
}
fn paired_kind(b: i32, next: i32) -> i32 {
    if b == 45 && next == 62 { return 27; }
    if b == 61 && next == 61 { return 35; }
    if b == 33 && next == 61 { return 36; }
    if b == 60 && next == 61 { return 38; }
    if b == 62 && next == 61 { return 40; }
    if b == 38 && next == 38 { return 41; }
    if b == 124 && next == 124 { return 42; }
    return 0;
}
fn punctuation(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let b = code(&*c.source, start);
    let mut kind = single_kind(b);
    let mut width = 1;
    if start + 1 < size {
        let paired = paired_kind(b, code(&*c.source, start + 1));
        if paired != 0 { kind = paired; width = 2; }
    }
    if kind == 0 { c.error = 65; c.error_start = start; return start; }
    let end = start + width;
    push(&mut *c, kind, start, end, 0);
    return end;
}
fn scan_one(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let b = code(&*c.source, start);
    if b == 32 || (b >= 9 && b <= 13) { return start + 1; }
    if b == 47 && start + 1 < size {
        let next = code(&*c.source, start + 1);
        if next == 47 {
            let mut i = start + 2;
            while i < size && code(&*c.source, i) != 10 { i = i + 1; }
            return i;
        }
        if next == 42 { return block_comment(&mut *c, start, size); }
    }
    if letter(b) { return identifier(&mut *c, start, size); }
    if digit(b) { return integer(&mut *c, start, size); }
    return punctuation(&mut *c, start, size);
}
pub fn scan(c: &mut Compiler) -> () {
    let size = bytes_len(&*c.source);
    if size > 65536 { c.error = 68; return; }
    let mut i = 0;
    while i < size {
        if code(&*c.source, i) > 127 { c.error = 65; c.error_start = i; return; }
        i = i + 1;
    }
    i = 0;
    while i < size && c.error == 0 { i = scan_one(&mut *c, i, size); }
    if c.error == 0 { push(&mut *c, 0, size, size, 0); }
    return;
}
