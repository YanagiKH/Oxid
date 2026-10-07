mod arena;
mod scanner;
mod parser;
mod evaluator;
mod stack_code;
mod lowering;
mod artifact_writer;

use std::io::read_stdin;
use std::io::ReadStatus;

fn produce_prefix(codes: &[i32], used: i32) -> i32 {
    let mut nodes = crate::arena::new_arena();
    let parsed = crate::parser::parse_prefix(&*codes, used, &mut nodes);
    match parsed {
        crate::parser::ParseResult::Parsed(root) => {
            let mut code = crate::stack_code::new_code();
            let lowered = crate::lowering::lower(&nodes, root, &mut code);
            match lowered {
                crate::lowering::LowerResult::Lowered(count) => {
                    return crate::artifact_writer::emit(&code);
                },
                crate::lowering::LowerResult::InvalidArena(position) => { return 65; },
            }
        },
        crate::parser::ParseResult::ExpectedOperand(position) => { return 64; },
        crate::parser::ParseResult::ExpectedOperator(position) => { return 64; },
        crate::parser::ParseResult::UnexpectedClose(position) => { return 64; },
        crate::parser::ParseResult::UnclosedParen(position) => { return 64; },
        crate::parser::ParseResult::InvalidCharacter(position) => { return 64; },
        crate::parser::ParseResult::NodeLimit(position) => { return 64; },
        crate::parser::ParseResult::OperatorLimit(position) => { return 64; },
        crate::parser::ParseResult::OperandLimit(position) => { return 64; },
        crate::parser::ParseResult::InputLimit(position) => { return 64; },
    }
}

fn main() -> i32 {
    // Unchanged 128-byte expression cap plus one capacity witness.
    let mut codes = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0,
    ];
    let input = read_stdin(&mut codes);
    match input {
        ReadStatus::Eof(used) => {
            if used < 0 || used > 128 { return 64; }
            return produce_prefix(&codes, used);
        },
        ReadStatus::Full => { return 64; },
        ReadStatus::IoError => { return 74; },
    }
}
