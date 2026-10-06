mod arena;
mod scanner;
mod parser;
mod evaluator;

fn main() -> i32 {
    let codes = [49, 50, 32, 43, 32, 51, 32, 42, 32, 40, 52, 32, 43, 32, 53, 41];
    let mut nodes = crate::arena::new_arena();
    let parsed = crate::parser::parse(&codes, &mut nodes);
    match parsed {
        crate::parser::ParseResult::Parsed(root) => {
            if nodes.count != 7 { return -2; }
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
        crate::parser::ParseResult::InputLimit(position) => { return -1; },
    }
}
