// AST3 raw validation precedes lossless normalization. No host authority.
use std::io::read_stdin;
use std::io::ReadStatus;
pub struct Header { pub used: i32, pub rows: i32, pub items: i32 }
fn exact(bytes: &mut [i32]) -> i32 {
    let result = read_stdin(&mut *bytes);
    match result { ReadStatus::Full => { return 0; }, ReadStatus::Eof(count) => { return 64; }, ReadStatus::IoError => { return 74; }, }
}
fn column(headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], meta: &Header, mode: i32) -> i32 {
    let mut low = crate::buffers::zeros();
    let low_status = exact(&mut low); if low_status != 0 { return low_status; }
    let mut high = crate::buffers::zeros();
    let high_status = exact(&mut high); if high_status != 0 { return high_status; }
    let mut i = 0;
    while i < 129 {
        let value = low[i] + 256 * high[i];
        if i >= meta.rows && value != 0 { return 64; }
        if mode == 0 {
            if value % 64 > 36 || value / 64 > meta.rows || (i < meta.rows && value % 64 == 0) { return 64; }
            headers[i] = value % 64; ab[i] = value / 64 * 65536;
        } else { if mode == 1 {
            if value > meta.used { return 64; }
            headers[i] = headers[i] + 64 * value;
        } else { if mode == 2 {
            if value > meta.used || value < headers[i] / 64 { return 64; }
            headers[i] = headers[i] + 32768 * value;
        } else {
            if value % 256 > 128 || value / 256 > 128 { return 64; }
            if mode == 3 { ab[i] = ab[i] + value; } else { cd[i] = value; }
        } } }
        i = i + 1;
    }
    return 0;
}
pub fn read(codes: &mut [i32], headers: &mut [i32], ab: &mut [i32], cd: &mut [i32], meta: &mut Header) -> i32 {
    let mut outer = [0, 0, 0, 0, 0, 0];
    let first = exact(&mut outer); if first != 0 { return first; }
    if outer[0] != 65 || outer[1] != 83 || outer[2] != 84 || outer[3] != 51 { return 64; }
    meta.used = outer[4] + 256 * outer[5]; if meta.used > 511 { return 64; }
    let mut byte = [0]; let mut i = 0;
    let mut opa = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    // Reuse the future OPA header owner for full14-byte source chunks.
    // Never read across the declared source/OPA boundary. Final tail is scalar.
    while i + 14 <= meta.used {
        let chunk_status = exact(&mut opa); if chunk_status != 0 { return chunk_status; }
        let mut j = 0;
        while j < 14 {
            if opa[j] > 127 { return 64; }
            crate::source_bytes::put(&mut *codes, i + j, opa[j]);
            j = j + 1;
        }
        i = i + 14;
    }
    while i < meta.used {
        let status = exact(&mut byte); if status != 0 { return status; }
        if byte[0] > 127 { return 64; }
        crate::source_bytes::put(&mut *codes, i, byte[0]); i = i + 1;
    }
    let header = exact(&mut opa); if header != 0 { return header; }
    if opa[0] != 79 || opa[1] != 80 || opa[2] != 65 || opa[3] != 51 || opa[4] != 0 || opa[5] != 0 || opa[6] != 0 || opa[7] != 0 || opa[8] != 0 || opa[9] != 0 || opa[10] > 128 || opa[11] > opa[10] || opa[12] + 256 * opa[13] != meta.used { return 64; }
    if (opa[10] == 0) != (opa[11] == 0) { return 64; }
    meta.rows = opa[10]; meta.items = opa[11];
    let mut mode = 0;
    while mode < 5 {
        let status = column(&mut *headers, &mut *ab, &mut *cd, &*meta, mode); if status != 0 { return status; }
        mode = mode + 1;
    }
    let final_read = read_stdin(&mut byte);
    match final_read { ReadStatus::Eof(count) => { if count == 0 { return 0; } return 64; }, ReadStatus::Full => { return 64; }, ReadStatus::IoError => { return 74; }, }
}
