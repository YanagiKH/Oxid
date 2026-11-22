use crate::model::Compiler;
use crate::model::Token;
use crate::model::grown;
use crate::model::word;
use crate::model::code;
use alloc::bytes::len as bytes_len;
use alloc::bytes::try_push as push_byte;
use alloc::vec::len;
use alloc::vec::try_push;

#[derive(Copy)]
struct Scalar { value: i32, end: i32 }

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
    let byte_type = b"u8";
    let long_type = b"i64";
    let module_word = b"mod";
    let public_word = b"pub";
    let crate_word = b"crate";
    let use_word = b"use";
    let as_word = b"as";
    let struct_word = b"struct";
    let for_word = b"for";
    let in_word = b"in";
    let char_word = b"char";
    let unsigned_word = b"u32";

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
    if word(&*source, start, end, &byte_type) { return 16; }
    if word(&*source, start, end, &module_word) { return 18; }
    if word(&*source, start, end, &public_word) { return 19; }
    if word(&*source, start, end, &crate_word) { return 44; }
    if word(&*source, start, end, &use_word) { return 46; }
    if word(&*source, start, end, &as_word) { return 47; }
    if word(&*source, start, end, &long_type) { return 48; }
    if word(&*source, start, end, &struct_word) { return 54; }
    if word(&*source, start, end, &for_word) { return 55; }
    if word(&*source, start, end, &in_word) { return 56; }
    if word(&*source, start, end, &char_word) { return 58; }
    if word(&*source, start, end, &unsigned_word) { return 60; }
    return pattern_keyword(&*source, start, end);
}
fn pattern_keyword(source: &[u8], start: i32, end: i32) -> i32 {
    let enum_word = b"enum";
    let match_word = b"match";
    if word(&*source, start, end, &enum_word) { return 62; }
    if word(&*source, start, end, &match_word) { return 63; }
    return 1;
}
fn push(c: &mut Compiler, kind: i32, start: i32, end: i32, value: i64) -> () {
    if len(&*c.tokens) >= 32768 { c.error = 68; return; }
    let token = Token { kind: kind, start: start, end: end, value: value, decoded_start: 0, decoded_length: 0 };
    if !grown(try_push(&mut *c.tokens, token)) {
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
    push(&mut *c, keyword_kind, start, i, 0i64);
    return i;
}
fn suffix_end(c: &mut Compiler, start: i32, end: i32, size: i32) -> bool {
    if end < size {
        let next = code(&*c.source, end);
        if letter(next) || digit(next) { c.error = 65; c.error_start = start; return false; }
    }
    return true;
}
fn integer(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let byte_suffix = b"u8";
    let long_suffix = b"i64";
    let unsigned_suffix = b"u32";
    let mut i = start;
    // Accumulate a negative magnitude so the signed minimum can be represented.
    let mut value = 0i64;
    while i < size && digit(code(&*c.source, i)) {
        let digit_value = code(&*c.source, i) - 48;
        let d = digit_value.to_i64();
        if value < -922337203685477580i64 || (value == -922337203685477580i64 && d > 8i64) {
            c.error = 65; c.error_start = start; return i;
        }
        value = value * 10i64 - d;
        i = i + 1;
    }
    if i + 2 <= size && word(&*c.source, i, i + 2, &byte_suffix) {
        let end = i + 2;
        if !suffix_end(&mut *c, start, end, size) { return end; }
        if value < -255i64 { c.error = 65; c.error_start = start; return end; }
        push(&mut *c, 17, start, end, 0i64 - value);
        return end;
    }
    if i + 3 <= size && word(&*c.source, i, i + 3, &long_suffix) {
        let end = i + 3;
        if suffix_end(&mut *c, start, end, size) { push(&mut *c, 49, start, end, value); }
        return end;
    }
    if i + 3 <= size && word(&*c.source, i, i + 3, &unsigned_suffix) {
        let end = i + 3;
        if !suffix_end(&mut *c, start, end, size) { return end; }
        if value < -4294967295i64 { c.error = 65; c.error_start = start; return end; }
        push(&mut *c, 61, start, end, 0i64 - value);
        return end;
    }
    if value < -2147483648i64 { c.error = 65; c.error_start = start; return i; }
    push(&mut *c, 2, start, i, value);
    return i;
}
fn hex(value: i32) -> i32 {
    if value >= 48 && value <= 57 { return value - 48; }
    if value >= 65 && value <= 70 { return value - 65 + 10; }
    if value >= 97 && value <= 102 { return value - 97 + 10; }
    return -1;
}

fn decoded(c: &mut Compiler, value: i32) -> bool {
    if !grown(push_byte(&mut *c.literals, value.to_u8_checked())) { c.error = 68; return false; }
    return true;
}

fn utf8(c: &mut Compiler, scalar: i32) -> bool {
    if scalar > 1114111 || (scalar >= 55296 && scalar <= 57343) { return false; }
    if scalar <= 127 { return decoded(&mut *c, scalar); }
    if scalar <= 2047 {
        return decoded(&mut *c, 192 + scalar / 64) && decoded(&mut *c, 128 + scalar % 64);
    }
    if scalar <= 65535 {
        return decoded(&mut *c, 224 + scalar / 4096) && decoded(&mut *c, 128 + scalar / 64 % 64)
            && decoded(&mut *c, 128 + scalar % 64);
    }
    return decoded(&mut *c, 240 + scalar / 262144) && decoded(&mut *c, 128 + scalar / 4096 % 64)
        && decoded(&mut *c, 128 + scalar / 64 % 64) && decoded(&mut *c, 128 + scalar % 64);
}

fn invalid_scalar() -> Scalar { return Scalar { value: -1, end: -1 }; }

fn valid_scalar(value: i32) -> bool {
    return value >= 0 && value <= 1114111 && (value < 55296 || value > 57343);
}

fn unicode_escape(c: &Compiler, start: i32, size: i32) -> Scalar {
    if start >= size || code(&*c.source, start) != 123 { return invalid_scalar(); }
    let mut i = start + 1;
    let mut scalar = 0;
    while i < size && code(&*c.source, i) != 125 {
        if i - start > 6 { return invalid_scalar(); }
        let digit_value = hex(code(&*c.source, i));
        if digit_value < 0 { return invalid_scalar(); }
        scalar = scalar * 16 + digit_value;
        i = i + 1;
    }
    if i == start + 1 || i >= size || !valid_scalar(scalar) { return invalid_scalar(); }
    return Scalar { value: scalar, end: i + 1 };
}

fn hex_escape(c: &Compiler, start: i32, size: i32) -> i32 {
    if start + 1 >= size { return -1; }
    let high = hex(code(&*c.source, start));
    let low = hex(code(&*c.source, start + 1));
    if high < 0 || low < 0 { return -1; }
    return high * 16 + low;
}

fn basic_escape(escaped: i32) -> i32 {
    if escaped == 110 { return 10; }
    if escaped == 114 { return 13; }
    if escaped == 116 { return 9; }
    if escaped == 48 { return 0; }
    if escaped == 34 || escaped == 92 { return escaped; }
    return -1;
}

fn escape(c: &mut Compiler, start: i32, size: i32) -> i32 {
    if start >= size { return -1; }
    let escaped = code(&*c.source, start);
    if escaped == 117 {
        let scalar = unicode_escape(&*c, start + 1, size);
        if scalar.end < 0 || !utf8(&mut *c, scalar.value) { return -1; }
        return scalar.end;
    }
    if escaped == 120 {
        let value = hex_escape(&*c, start + 1, size);
        if value < 0 || !decoded(&mut *c, value) { return -1; }
        return start + 3;
    }
    let value = basic_escape(escaped);
    if value < 0 || !decoded(&mut *c, value) { return -1; }
    return start + 1;
}

fn raw_scalar(c: &Compiler, start: i32, size: i32) -> Scalar {
    let first = code(&*c.source, start);
    if first < 128 { return Scalar { value: first, end: start + 1 }; }
    if first < 194 || first > 244 { return invalid_scalar(); }
    let mut width = 2;
    let mut value = first - 192;
    let mut minimum = 128;
    if first >= 224 { width = 3; value = first - 224; minimum = 2048; }
    if first >= 240 { width = 4; value = first - 240; minimum = 65536; }
    let end = start + width;
    if end > size { return invalid_scalar(); }
    for i in start + 1..end {
        let next = code(&*c.source, i);
        if next < 128 || next > 191 { return invalid_scalar(); }
        value = value * 64 + next - 128;
    }
    if value < minimum || !valid_scalar(value) { return invalid_scalar(); }
    return Scalar { value: value, end: end };
}

fn character_scalar(c: &Compiler, start: i32, size: i32) -> Scalar {
    if start >= size { return invalid_scalar(); }
    let first = code(&*c.source, start);
    if first != 92 {
        let scalar = raw_scalar(&*c, start, size);
        if scalar.value < 32 || (scalar.value >= 127 && scalar.value <= 159)
            || scalar.value == 39 { return invalid_scalar(); }
        return scalar;
    }
    let escape_start = start + 1;
    if escape_start >= size { return invalid_scalar(); }
    let escaped = code(&*c.source, escape_start);
    if escaped == 117 { return unicode_escape(&*c, escape_start + 1, size); }
    if escaped == 120 {
        let value = hex_escape(&*c, escape_start + 1, size);
        if value < 0 || value > 127 { return invalid_scalar(); }
        return Scalar { value: value, end: escape_start + 3 };
    }
    let mut value = basic_escape(escaped);
    if escaped == 39 { value = 39; }
    if value < 0 { return invalid_scalar(); }
    return Scalar { value: value, end: escape_start + 1 };
}

fn character(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let scalar = character_scalar(&*c, start + 1, size);
    if scalar.end < 0 || scalar.end >= size || code(&*c.source, scalar.end) != 39 {
        c.error = 65;
        c.error_start = start;
        return start;
    }
    let end = scalar.end + 1;
    let value = scalar.value;
    push(&mut *c, 59, start, end, value.to_i64());
    return end;
}

fn byte_string(c: &mut Compiler, start: i32, size: i32) -> i32 {
    let mut i = start + 2;
    let first = bytes_len(&*c.literals);
    while i < size {
        let value = code(&*c.source, i);
        if value == 34 {
            let length = bytes_len(&*c.literals) - first;
            if len(&*c.tokens) >= 32768 { c.error = 68; return i; }
            let row = Token { kind: 50, start: start, end: i + 1, value: 0i64,
                decoded_start: first, decoded_length: length };
            if !grown(try_push(&mut *c.tokens, row)) { c.error = 68; }
            return i + 1;
        }
        if value == 10 || value == 13 { break; }
        if value == 92 {
            let end = escape(&mut *c, i + 1, size);
            if end < 0 { break; }
            i = end;
        } else {
            if !decoded(&mut *c, value) { return i; }
            i = i + 1;
        }
        if bytes_len(&*c.literals) - first > 1024 { c.error = 68; return i; }
    }
    if c.error == 0 { c.error = 65; c.error_start = start; }
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
    if b == 46 { return 43; }
    if b == 91 { return 51; }
    if b == 93 { return 52; }
    if b == 35 { return 53; }
    if b == 38 { return 65; }
    if b == 124 { return 66; }
    if b == 94 { return 67; }
    if b == 126 { return 68; }
    return 0;
}
fn paired_kind(b: i32, next: i32) -> i32 {
    if b == 45 && next == 62 { return 27; }
    if b == 61 && next == 61 { return 35; }
    if b == 61 && next == 62 { return 64; }
    if b == 33 && next == 61 { return 36; }
    if b == 60 && next == 61 { return 38; }
    if b == 62 && next == 61 { return 40; }
    if b == 38 && next == 38 { return 41; }
    if b == 124 && next == 124 { return 42; }
    if b == 58 && next == 58 { return 45; }
    if b == 46 && next == 46 { return 57; }
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
    push(&mut *c, kind, start, end, 0i64);
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
    if b == 98 && start + 1 < size && code(&*c.source, start + 1) == 34 {
        return byte_string(&mut *c, start, size);
    }
    if b == 39 { return character(&mut *c, start, size); }
    if letter(b) { return identifier(&mut *c, start, size); }
    if digit(b) { return integer(&mut *c, start, size); }
    return punctuation(&mut *c, start, size);
}
pub fn scan_range(c: &mut Compiler, start: i32, size: i32) -> () {
    if size > 65536 { c.error = 68; return; }
    let mut i = start;
    while i < size {
        let scalar = raw_scalar(&*c, i, size);
        if scalar.end < 0 { c.error = 65; c.error_start = i; return; }
        i = scalar.end;
    }
    i = start;
    while i < size && c.error == 0 { i = scan_one(&mut *c, i, size); }
    if c.error == 0 { push(&mut *c, 0, size, size, 0i64); }
    return;
}

pub fn scan(c: &mut Compiler) -> () {
    let size = bytes_len(&*c.source);
    scan_range(&mut *c, 0, size);
    return;
}
