// Static tag 0 is available only to the root after complete resolution and typing.
pub fn emit(resolved: &[i32], semantic: &[i32], state: &crate::static_state::State, rows: i32, used: i32) -> i32 {
    if rows < 0 || rows > 128 || resolved.len() != 129 || semantic.len() != 129 || resolved[128] != 0 || semantic[128] != 0 || state.stack < 0 || state.stack >= 8388608 { return 70; }
    let kind = state.stack % 16;
    let expected = state.stack / 16 % 256;
    let actual = state.stack / 4096 % 256;
    let label = state.stack / 1048576;
    let start = crate::parser_state::lo(state.locals);
    let end = crate::parser_state::hi(state.locals);
    let first = crate::parser_state::lo(state.value);
    let last = crate::parser_state::hi(state.value);
    let mut tag = 0;
    let mut expected_type = 0;
    let mut actual_type = 0;
    let mut expected_count = 0;
    let mut actual_count = 0;
    if kind == 0 {
        if state.stack != 0 || state.locals != 0 || state.value != 0 { return 70; }
    } else {
        if kind > 14 || start >= end || end > used || last > used { return 70; }
        if kind == 8 { expected_type = expected; actual_type = actual; }
        if kind == 10 { expected_count = expected; actual_count = actual; }
        tag = 1;
    }
    let header = [83, 84, 70, 50, tag, rows, kind, start, end, first, last, label, expected_type, actual_type, expected_count, actual_count];
    let first_write = crate::static_column::write(&header);
    if first_write != 0 || tag == 1 { return first_write; }
    let result = crate::static_column::column(&*resolved);
    if result != 0 { return result; }
    return crate::static_column::column(&*semantic);
}
