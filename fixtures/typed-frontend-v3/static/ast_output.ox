// Exact OPA3 echo of already validated lossless columns.
pub fn emit(headers: &[i32], ab: &[i32], cd: &[i32], meta: &crate::ast_input::Header) -> i32 {
    if headers.len() != 129 || ab.len() != 129 || cd.len() != 129 || meta.used < 0 || meta.used > 511 || meta.rows < 0 || meta.rows > 128 || meta.items < 0 || meta.items > meta.rows || (meta.rows == 0) != (meta.items == 0) { return 70; }
    let header = [79, 80, 65, 51, 0, 0, 0, 0, 0, 0, meta.rows, meta.items, meta.used % 256, meta.used / 256];
    let first = crate::static_column::write(&header); if first != 0 { return first; }
    let mut bytes = crate::buffers::zeros(); let mut mode = 0;
    while mode < 5 {
        let mut divisor = 1; let mut plane = 0;
        while plane < 2 {
            let mut i = 0;
            while i < 129 {
                let mut value = cd[i];
                if mode == 0 { value = headers[i] % 64 + 64 * (ab[i] / 65536); }
                if mode == 1 { value = crate::static_span::lo(headers[i]); }
                if mode == 2 { value = crate::static_span::hi(headers[i]); }
                if mode == 3 { value = ab[i] % 65536; }
                bytes[i] = value / divisor % 256; i = i + 1;
            }
            let status = crate::static_column::write(&bytes); if status != 0 { return status; }
            divisor = 256; plane = plane + 1;
        }
        mode = mode + 1;
    }
    return 0;
}
