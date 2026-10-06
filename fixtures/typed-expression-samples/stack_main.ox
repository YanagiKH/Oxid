mod arena;
mod scanner;
mod parser;
mod evaluator;
mod stack_code;
mod lowering;


fn evaluate_prefix(codes: &[i32], used: i32) -> i32 {
    let mut nodes = crate::arena::new_arena();
    let parsed = crate::parser::parse_prefix(&*codes, used, &mut nodes);
    match parsed {
        crate::parser::ParseResult::Parsed(root) => {
            let mut code = crate::stack_code::new_code();
            let lowered = crate::lowering::lower(&nodes, root, &mut code);
            match lowered {
                crate::lowering::LowerResult::Lowered(count) => {
                    let result = crate::stack_code::execute(&code);
                    match result {
                        crate::stack_code::CodeResult::Value(value) => { return value; },
                        crate::stack_code::CodeResult::InvalidCode(position) => { return -6; },
                    }
                },
                crate::lowering::LowerResult::InvalidArena(position) => { return -3; },
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
    let codes = [49, 50, 32, 43, 32, 51, 32, 42, 32, 40, 52, 32, 43, 32, 53, 41];
    return evaluate_prefix(&codes, 16);
}
