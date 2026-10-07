// Permanent precursor-only emitter: no static-success or diagnostic tag path.
pub fn emit_probe(resolved: &[i32], semantic: &[i32], rows: i32) -> i32 {
    if rows < 0 || rows > 128 || resolved.len() != 129 || semantic.len() != 129 || resolved[128] != 0 || semantic[128] != 0 { return 70; }
    let header = [83, 84, 70, 49, 2, rows, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let first = crate::static_column::write(&header); if first != 0 { return first; }
    let a = crate::static_column::column(&*resolved); if a != 0 { return a; }
    return crate::static_column::column(&*semantic);
}
