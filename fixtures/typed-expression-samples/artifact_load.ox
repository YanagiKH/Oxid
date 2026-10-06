mod artifact_reader;

use std::io::read_stdin;
use std::io::ReadStatus;

fn main() -> i32 {
    // Exactly 80 artifact bytes plus one explicit trailing-byte witness.
    // Default result mode preserves all nonnegative i32 expression results.
    let mut bytes = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0,
    ];
    let input = read_stdin(&mut bytes);
    match input {
        ReadStatus::Eof(used) => {
            if used != 80 { return -4; }
            let result = crate::artifact_reader::load(&bytes, used);
            match result {
                crate::artifact_reader::LoadResult::Value(value) => { return value; },
                crate::artifact_reader::LoadResult::InvalidArtifact(position) => { return -6; },
            }
        },
        ReadStatus::Full => { return -4; },
        ReadStatus::IoError => { return -5; },
    }
}
