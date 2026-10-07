// A resolved program remains pending typing. It can never emit static tag 0.
use std::io::write_stdout;
use std::io::WriteStatus;
pub fn emit(resolved: &[i32], semantic: &[i32], state: &crate::static_state::State, rows: i32, used: i32) -> i32 {
    if state.stack == 0 { return crate::static_output::emit_probe(&*resolved, &*semantic, rows); }
    let kind = state.stack;
    let start = crate::parser_state::lo(state.locals);
    let end = crate::parser_state::hi(state.locals);
    let first = crate::parser_state::lo(state.value);
    let last = crate::parser_state::hi(state.value);
    if kind < 1 || kind > 7 || start >= end || end > used || rows < 0 || rows > 128 { return 70; }
    let mut label = 0;
    if kind == 3 {
        if first >= last || last > used { return 70; }
        label = 1;
    } else { if state.value != 0 { return 70; } }
    let header = [83, 84, 70, 49, 1, rows, kind, start, end, first, last, label, 0, 0, 0, 0];
    let result = write_stdout(&header);
    match result {
        WriteStatus::Complete => { return 0; },
        WriteStatus::InvalidInput => { return 70; },
        WriteStatus::IoError(accepted) => { return 74; },
    }
}
