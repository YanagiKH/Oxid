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
fn column(values: &[i32]) -> i32 {
    let mut bytes = crate::buffers::zeros();
    let mut divisor = 1;
    let mut plane = 0;
    while plane < 2 {
        let mut i = 0;
        while i < 129 { bytes[i] = values[i] / divisor % 256; i = i + 1; }
        let result = write(&bytes);
        if result != 0 { return result; }
        plane = plane + 1;
        if plane < 2 { divisor = divisor * 256; }
    }
    return 0;
}
fn valid(row_starts: &[i32], row_ends: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], state: &crate::parser_state::State) -> bool {
    if (state.rows == 0 && state.items != 0) || (state.rows != 0 && state.items == 0) { return false; }
    if state.rows < 0 || state.rows > 128 || state.items < 0 || state.items > state.rows { return false; }
    let mut i = 0;
    while i < 129 {
        let h = headers[i];
        let a = ab[i];
        let b = cd[i];
        if i >= state.rows {
            if h != 0 || a != 0 || b != 0 || row_starts[i] != 0 || row_ends[i] != 0 { return false; }
        } else {
            if h < 0 || h % 64 < 1 || h % 64 > 36 || h / 64 > state.rows { return false; }
            let start = row_starts[i];
            let end = row_ends[i];
            if start < 0 || start > end || end > state.used || end > 511 { return false; }
            if a < 0 || a % 256 > 128 || a / 256 > 128 || b < 0 || b % 256 > 128 || b / 256 > 128 { return false; }
        }
        i = i + 1;
    }
    return true;
}
pub fn lexical(used: i32, start: i32, detail: i32) -> i32 {
    if detail < 1 || detail > 2 { return 70; }
    if used < 0 || used > 511 || start < 0 || start > used { return 64; }
    let header = [79, 80, 65, 51, 6, detail, start % 256, start / 256, used % 256, used / 256, 0, 0, used % 256, used / 256];
    return write(&header);
}
pub fn emit(row_starts: &[i32], row_ends: &[i32], headers: &[i32], ab: &[i32], cd: &[i32], state: &crate::parser_state::State) -> i32 {
    // Validate all source-bound diagnostic fields before any output.
    if state.used < 0 || state.used > 511 || state.start < 0 || state.end < state.start || state.end > state.used || state.end > 511 { return 64; }
    if state.error < 0 || state.error > 6 || state.detail < 0 || state.detail > 255 { return 70; }
    if state.error == 9 { return 70; }
    if state.error != 0 {
        if state.detail == 0 { return 70; }
        let failure = [79, 80, 65, 51, state.error, state.detail, state.start % 256, state.start / 256, state.end % 256, state.end / 256, 0, 0, state.used % 256, state.used / 256];
        return write(&failure);
    }
    if state.detail != 0 || state.start != 0 || state.end != 0 { return 70; }
    if !valid(&*row_starts, &*row_ends, &*headers, &*ab, &*cd, &*state) { return 70; }
    let header = [79, 80, 65, 51, 0, 0, 0, 0, 0, 0, state.rows, state.items, state.used % 256, state.used / 256];
    let first = write(&header); if first != 0 { return first; }
    let a = column(&*headers); if a != 0 { return a; }
    let lo = column(&*row_starts); if lo != 0 { return lo; }
    let hi = column(&*row_ends); if hi != 0 { return hi; }
    let b = column(&*ab); if b != 0 { return b; }
    return column(&*cd);
}
