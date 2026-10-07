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
    while i < 129 { bytes[i] = values[i] / 256 % 256; i = i + 1; }
    let middle = write(&bytes);
    if middle != 0 { return middle; }
    i = 0;
    while i < 129 { bytes[i] = values[i] / 65536 % 256; i = i + 1; }
    let high = write(&bytes);
    if high != 0 { return high; }
    i = 0;
    while i < 129 { bytes[i] = values[i] / 16777216; i = i + 1; }
    return write(&bytes);
}
pub fn emit(headers: &[i32], ab: &[i32], cd: &[i32], count: i32) -> i32 {
    if !crate::parser_banks::valid(&*headers, &*ab, &*cd, count) { return 70; }
    let header = [80, 65, 66, 51, 128];
    let h = write(&header); if h != 0 { return h; }
    let a = column(&*headers); if a != 0 { return a; }
    let b = column(&*ab); if b != 0 { return b; }
    return column(&*cd);
}
