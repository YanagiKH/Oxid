// Permanent precursor-only emitter: no static-success or diagnostic tag path.
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
        while i < 129 {
            // Correct truncating division to floor without ever negating MIN.
            let value = values[i];
            let mut quotient = value / divisor;
            if value % divisor < 0 { quotient = quotient - 1; }
            let mut byte = quotient % 256;
            if byte < 0 { byte = byte + 256; }
            bytes[i] = byte;
            i = i + 1;
        }
        let result = write(&bytes); if result != 0 { return result; }
        plane = plane + 1;
        if plane < 4 { divisor = divisor * 256; }
    }
    return 0;
}
pub fn emit_probe(resolved: &[i32], semantic: &[i32], rows: i32) -> i32 {
    if rows < 0 || rows > 128 || resolved.len() != 129 || semantic.len() != 129 || resolved[128] != 0 || semantic[128] != 0 { return 70; }
    let header = [83, 84, 70, 49, 2, rows, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let first = write(&header); if first != 0 { return first; }
    let a = column(&*resolved); if a != 0 { return a; }
    return column(&*semantic);
}
