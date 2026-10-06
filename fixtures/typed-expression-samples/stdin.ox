mod arena;
mod scanner;
mod parser;
mod evaluator;

use std::io::read_stdin;
use std::io::ReadStatus;

fn evaluate_prefix(codes: &[i32], used: i32) -> i32 {
    let mut nodes = crate::arena::new_arena();
    let parsed = crate::parser::parse_prefix(&*codes, used, &mut nodes);
    match parsed {
        crate::parser::ParseResult::Parsed(root) => {
            let result = crate::evaluator::evaluate(&nodes, root);
            match result {
                crate::evaluator::EvalResult::Value(value) => { return value; },
                crate::evaluator::EvalResult::InvalidArena(position) => { return -3; },
            }
        },
        crate::parser::ParseResult::ExpectedOperand(position) => { return -1; },
        crate::parser::ParseResult::ExpectedOperator(position) => { return -1; },
        crate::parser::ParseResult::UnexpectedClose(position) => { return -1; },
        crate::parser::ParseResult::UnclosedParen(position) => { return -1; },
        crate::parser::ParseResult::InvalidCharacter(position) => { return -1; },
        crate::parser::ParseResult::NodeLimit(position) => { return -1; },
        crate::parser::ParseResult::OperatorLimit(position) => { return -1; },
        crate::parser::ParseResult::OperandLimit(position) => { return -1; },
        crate::parser::ParseResult::InputLimit(position) => { return -4; },
    }
}

fn main() -> i32 {
    // 128 input cells plus one explicit over-capacity witness. Unused zero
    // cells are not whitespace and must never be scanned after Eof(used).
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
            if used < 0 || used > 128 { return -4; }
            return evaluate_prefix(&codes, used);
        },
        ReadStatus::Full => { return -4; },
        ReadStatus::IoError => { return -5; },
    }
}
