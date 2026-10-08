// OPA2 syntax-success writer for the AST2 consumer, after complete validation.
// This is serialization only, not a source/AST or semantic authority constructor.
// The closed consumer imports this paid module but does not invoke it.
pub fn emit(headers: &[i32], ab: &[i32], cd: &[i32], meta: &crate::ast_input::Header) -> i32 {
    if headers.len() != 129 || ab.len() != 129 || cd.len() != 129 || meta.used < 0 || meta.used > 255 || meta.rows < 0 || meta.rows > 128 || meta.items < 0 || meta.items > meta.rows || (meta.rows == 0) != (meta.items == 0) { return 70; }
    let header = [79, 80, 65, 50, 0, 0, 0, 0, meta.rows, meta.items, meta.used];
    let first = crate::static_column::write(&header);
    if first != 0 { return first; }
    let a = crate::static_column::column(&*headers);
    if a != 0 { return a; }
    let b = crate::static_column::column(&*ab);
    if b != 0 { return b; }
    return crate::static_column::column(&*cd);
}
