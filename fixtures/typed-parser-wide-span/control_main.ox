// Synthetic carrier/serializer controls, separate from the full grammar entry.
mod buffers; mod parser_state; mod parser_output;
use std::io::read_stdin;
use std::io::ReadStatus;
fn state() -> crate::parser_state::State {
    return crate::parser_state::State { cursor: 0, limit: 0, used: 255, rows: 0, stack: 0, items: 0, tail: 0, mode: 0, value: 0, depth: 0, context: 0, floor: 0, error: 0, detail: 0, start: 0, end: 0 };
}
fn roundtrip(endpoint: i32) -> i32 {
    let mut starts = crate::buffers::zeros();
    let mut ends = crate::buffers::zeros();
    let mut headers = crate::buffers::zeros();
    let mut frames = crate::buffers::zeros();
    let mut s = state();
    let mut i = 0;
    while i < 128 {
        let id = crate::parser_state::row(&mut starts, &mut ends, &mut headers, &mut s, 36, endpoint, endpoint);
        if id != i + 1 || s.error != 0 { return 1; }
        crate::parser_state::link(&mut headers, id, 128);
        if headers[i] != 36 + 64 * 128 || starts[i] != endpoint || ends[i] != endpoint { return 2; }
        crate::parser_state::span(&mut starts, &mut ends, id, 0, endpoint);
        if starts[i] != 0 || ends[i] != endpoint || headers[i] != 36 + 64 * 128 { return 3; }
        // Token handles reference their own physical slot, including EOF slot128.
        ends[128] = endpoint;
        if crate::parser_state::lo(&starts, 47 + 64 * i) != 0 || crate::parser_state::hi(&ends, 47 + 64 * i) != endpoint { return 4; }
        i = i + 1;
    }
    if crate::parser_state::hi(&ends, 47 + 64 * 128) != endpoint { return 5; }
    frames[0] = 1; s.stack = 1;
    i = 1;
    while i <= 128 {
        crate::parser_state::push(&mut frames, &mut s, 21, i, 1984);
        if s.error != 0 { return 6; }
        i = i + 1;
    }
    while i > 1 {
        i = i - 1;
        let frame = crate::parser_state::pop(&mut frames, &mut s);
        if frame != i * 65536 + 21 + 32 * 1984 || frames[i] != 0 { return 7; }
    }
    return 0;
}
fn serialize(mode: i32) -> i32 {
    let mut starts = crate::buffers::zeros();
    let mut ends = crate::buffers::zeros();
    let mut headers = crate::buffers::zeros();
    let mut ab = crate::buffers::zeros();
    let mut cd = crate::buffers::zeros();
    let mut frames = crate::buffers::zeros();
    let mut s = state();
    let mut i = 0;
    while i < 128 {
        let id = crate::parser_state::row(&mut starts, &mut ends, &mut headers, &mut s, 1, 255, 255);
        if i < 127 { crate::parser_state::link(&mut headers, id, id + 1); }
        i = i + 1;
    }
    s.items = 1;
    if mode == 6 { s.used = 256; }
    if mode == 7 { s.error = 1; s.detail = 11; s.start = 255; s.end = 256; }
    if mode == 8 { starts[0] = 256; ends[0] = 256; }
    if mode == 9 { starts[0] = -1; }
    if mode == 10 { starts[0] = 255; ends[0] = 254; }
    if mode == 11 { headers[128] = 1; }
    if mode == 12 { ends[128] = 1; }
    if mode == 13 { headers[0] = 1 + 64 * 129; }
    if mode == 14 { return crate::parser_output::lexical(256, 255, 1); }
    if mode == 15 { s.error = 1; s.detail = 11; s.start = 255; s.end = 255; }
    if mode == 17 {
        let refused = crate::parser_state::row(&mut starts, &mut ends, &mut headers, &mut s, 1, 0, 1);
        if refused != 0 || s.rows != 128 || s.error != 9 { return 1; }
    }
    if mode == 18 {
        s.stack = 1; frames[0] = 1;
        let refused = crate::parser_state::pop(&mut frames, &mut s);
        if refused != 0 || s.stack != 1 || s.error != 9 { return 1; }
    }
    if mode == 19 {
        s.stack = 129;
        crate::parser_state::push(&mut frames, &mut s, 21, 128, 1984);
        if s.stack != 129 || s.error != 9 { return 1; }
    }
    if mode == 20 { s.used = 0; s.error = 1; s.detail = 11; s.start = 0; s.end = 1; }
    if mode == 21 { s.used = 254; s.error = 1; s.detail = 11; s.start = 254; s.end = 255; }
    return crate::parser_output::emit(&starts, &ends, &headers, &ab, &cd, &s);
}
fn main() -> i32 {
    let mut input = [0];
    let result = read_stdin(&mut input);
    match result {
        ReadStatus::Full => {},
        ReadStatus::Eof(n) => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
    let mode = input[0];
    if mode < 6 {
        let endpoints = [254, 255, 256, 65535, 65536, 2147483647];
        return roundtrip(endpoints[mode]);
    }
    if mode > 21 { return 64; }
    return serialize(mode);
}
