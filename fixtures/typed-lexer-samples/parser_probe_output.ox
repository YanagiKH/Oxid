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
    let mut bytes = crate::tape::zeros();
    let mut i = 0;
    while i < 129 { bytes[i] = values[i] % 256; i = i + 1; }
    let low = write(&bytes);
    if low != 0 { return low; }
    i = 0;
    while i < 129 { bytes[i] = values[i] / 256; i = i + 1; }
    return write(&bytes);
}
pub fn emit(rows: &crate::parser_banks::Rows) -> i32 {
    if !crate::parser_banks::valid(&*rows) { return 70; }
    let header = [80, 65, 66, 49, 128];
    let h = write(&header); if h != 0 { return h; }
    let k = column(&*rows.kind); if k != 0 { return k; }
    let s = column(&*rows.span); if s != 0 { return s; }
    let a = column(&*rows.a); if a != 0 { return a; }
    let b = column(&*rows.b); if b != 0 { return b; }
    let c = column(&*rows.c); if c != 0 { return c; }
    let d = column(&*rows.d); if d != 0 { return d; }
    return column(&*rows.next);
}
