// Shared signed-column transport. No probe or semantic acceptance policy.
use std::io::write_stdout;
use std::io::WriteStatus;
pub fn write(bytes: &[i32]) -> i32 {
    let result = write_stdout(&*bytes);
    match result {
        WriteStatus::Complete => { return 0; },
        WriteStatus::InvalidInput => { return 70; },
        WriteStatus::IoError(accepted) => { return 74; },
    }
}
pub fn column(values: &[i32]) -> i32 {
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
