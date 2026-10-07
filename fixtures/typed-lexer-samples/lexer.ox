// Compatibility adapter for callers that store token columns in a Tape.
// Separate column owners preserve the existing whole-record borrow rule.
pub enum LexResult { Complete, UnterminatedString(i32), UnterminatedComment(i32) }

fn copy_columns(tokens: &mut crate::tape::Tape, kinds: &[i32], starts: &[i32], ends: &[i32]) -> () {
    let mut i = 0;
    while i < 129 {
        tokens.kind[i] = kinds[i];
        tokens.start[i] = starts[i];
        tokens.end[i] = ends[i];
        i = i + 1;
    }
    return;
}

pub fn scan(codes: &[i32], used: i32, tokens: &mut crate::tape::Tape) -> LexResult {
    let mut kinds = crate::buffers::zeros();
    let mut starts = crate::buffers::zeros();
    let mut ends = crate::buffers::zeros();
    let result = crate::lexer_core::scan(&*codes, used, &mut kinds, &mut starts, &mut ends);
    copy_columns(&mut *tokens, &kinds, &starts, &ends);
    match result {
        crate::lexer_core::ScanResult::Complete(count) => {
            tokens.count = count;
            return LexResult::Complete;
        },
        crate::lexer_core::ScanResult::UnterminatedString(start) => {
            tokens.count = 0;
            return LexResult::UnterminatedString(start);
        },
        crate::lexer_core::ScanResult::UnterminatedComment(start) => {
            tokens.count = 0;
            return LexResult::UnterminatedComment(start);
        },
    }
}
