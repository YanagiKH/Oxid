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
    while plane < 4 {
        let mut i = 0;
        while i < 129 { bytes[i] = values[i] / divisor % 256; i = i + 1; }
        let result = write(&bytes);
        if result != 0 { return result; }
        plane = plane + 1;
        if plane < 4 { divisor = divisor * 256; }
    }
    return 0;
}
fn valid(headers: &[i32], ab: &[i32], cd: &[i32], state: &crate::parser_state::State) -> bool {
    if state.rows < 0 || state.rows > 128 || state.items < 0 || state.items > state.rows { return false; }
    let mut i = 0;
    while i < 129 {
        let h = headers[i];
        let a = ab[i];
        let b = cd[i];
        if i >= state.rows {
            if h != 0 || a != 0 || b != 0 { return false; }
        } else {
            if h < 0 || h % 64 < 1 || h % 64 > 36 || h / 4194304 > state.rows { return false; }
            let start = crate::parser_state::lo(h);
            let end = crate::parser_state::hi(h);
            if start > end || end > state.used { return false; }
            if a < 0 || a % 256 > 128 || a / 256 > 128 || b < 0 || b % 256 > 128 || b / 256 > 128 { return false; }
        }
        i = i + 1;
    }
    return true;
}
pub fn lexical(used: i32, start: i32, detail: i32) -> i32 {
    let header = [79, 80, 65, 49, 6, detail, start, used, 0, 0, used];
    return write(&header);
}
pub fn emit(headers: &[i32], ab: &[i32], cd: &[i32], state: &crate::parser_state::State) -> i32 {
    if state.error == 9 { return 70; }
    if state.error != 0 {
        let failure = [79, 80, 65, 49, state.error, state.detail, state.start, state.end, 0, 0, state.used];
        return write(&failure);
    }
    if !valid(&*headers, &*ab, &*cd, &*state) { return 70; }
    let header = [79, 80, 65, 49, 0, 0, 0, 0, state.rows, state.items, state.used];
    let first = write(&header); if first != 0 { return first; }
    let a = column(&*headers); if a != 0 { return a; }
    let b = column(&*ab); if b != 0 { return b; }
    return column(&*cd);
}
