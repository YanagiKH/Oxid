// AST2 exact-byte transport only. No success authority is created here.
use std::io::read_stdin;
use std::io::ReadStatus;
pub struct Header { pub used: i32, pub rows: i32, pub items: i32 }
fn exact(bytes: &mut [i32]) -> i32 {
    let result = read_stdin(&mut *bytes);
    match result {
        ReadStatus::Full => { return 0; },
        ReadStatus::Eof(count) => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
fn column(values: &mut [i32], bytes: &mut [i32]) -> i32 {
    let mut plane = 0;
    let mut multiplier = 1;
    while plane < 4 {
        let status = exact(&mut *bytes);
        if status != 0 { return status; }
        let mut i = 0;
        while i < 129 {
            // Reject unsigned i32 overflow BEFORE multiplication or addition.
            if plane == 3 && bytes[i] >= 128 { return 64; }
            values[i] = values[i] + bytes[i] * multiplier;
            i = i + 1;
        }
        plane = plane + 1;
        if plane < 4 { multiplier = multiplier * 256; }
    }
    return 0;
}
pub fn read(codes: &mut [i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], meta: &mut Header) -> i32 {
    let mut outer = [0, 0, 0, 0, 0];
    let first = exact(&mut outer);
    if first != 0 { return first; }
    if outer[0] != 65 || outer[1] != 83 || outer[2] != 84 || outer[3] != 50 || outer[4] > 255 { return 64; }
    meta.used = outer[4];
    let mut byte = [0];
    let mut i = 0;
    while i < meta.used {
        let status = exact(&mut byte);
        if status != 0 { return status; }
        if byte[0] > 127 { return 64; }
        codes[i] = byte[0];
        i = i + 1;
    }
    let mut opa = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let header = exact(&mut opa);
    if header != 0 { return header; }
    if opa[0] != 79 || opa[1] != 80 || opa[2] != 65 || opa[3] != 50 || opa[4] != 0 || opa[5] != 0 || opa[6] != 0 || opa[7] != 0 || opa[8] > 128 || opa[9] > opa[8] || opa[10] != meta.used { return 64; }
    if (opa[8] == 0) != (opa[9] == 0) { return 64; }
    meta.rows = opa[8]; meta.items = opa[9];
    let mut bytes = crate::buffers::zeros();
    let a = column(&mut *headers, &mut bytes); if a != 0 { return a; }
    let b = column(&mut *ab, &mut bytes); if b != 0 { return b; }
    let c = column(&mut *cd, &mut bytes); if c != 0 { return c; }
    // Full means only that this positive-capacity buffer was filled.
    // One trailing byte suffices to reject; no further input is consumed.
    let final_read = read_stdin(&mut byte);
    match final_read {
        ReadStatus::Eof(count) => { if count == 0 { return 0; } return 64; },
        ReadStatus::Full => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
