pub enum ParseResult {
    Parsed(i32),
    ExpectedOperand(i32),
    ExpectedOperator(i32),
    UnexpectedClose(i32),
    UnclosedParen(i32),
    InvalidCharacter(i32),
    NodeLimit(i32),
    OperatorLimit(i32),
    OperandLimit(i32),
    InputLimit(i32),
}

// Operator codes are application data: 0 is a parenthesis marker,
// 2 is addition and 3 is multiplication. Larger codes bind more tightly.
struct Work {
    operators: [i32; 15],
    positions: [i32; 15],
    operands: [i32; 15],
    operator_count: i32,
    operand_count: i32,
    expect_operand: bool,
    error_position: i32,
}

fn new_work() -> Work {
    return Work {
        operators: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        positions: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        operands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        operator_count: 0,
        operand_count: 0,
        expect_operand: true,
        error_position: 0,
    };
}

// Internal statuses: 0 succeeds; 1/2 are missing operand/operator;
// 3/4 are unexpected close/unclosed open; 6/7/8 are node/operator/operand
// capacity failures. The public result is always constructed explicitly.
fn fail(work: &mut Work, status: i32, position: i32) -> i32 {
    work.error_position = position;
    return status;
}

fn failure(status: i32, position: i32) -> ParseResult {
    if status == 1 { return ParseResult::ExpectedOperand(position); }
    if status == 2 { return ParseResult::ExpectedOperator(position); }
    if status == 3 { return ParseResult::UnexpectedClose(position); }
    if status == 4 { return ParseResult::UnclosedParen(position); }
    if status == 6 { return ParseResult::NodeLimit(position); }
    if status == 7 { return ParseResult::OperatorLimit(position); }
    return ParseResult::OperandLimit(position);
}

fn push_operator(work: &mut Work, kind: i32, position: i32) -> i32 {
    let count = work.operator_count;
    if count >= 15 { return fail(&mut *work, 7, position); }
    work.operators[count] = kind;
    work.positions[count] = position;
    work.operator_count = count + 1;
    return 0;
}

fn literal(work: &mut Work, arena: &mut crate::arena::Arena, value: i32, position: i32) -> i32 {
    if !work.expect_operand { return fail(&mut *work, 2, position); }
    let node = crate::arena::append(&mut *arena, 1, value, -1, -1);
    if node < 0 { return fail(&mut *work, 6, position); }
    let count = work.operand_count;
    if count >= 15 { return fail(&mut *work, 8, position); }
    work.operands[count] = node;
    work.operand_count = count + 1;
    work.expect_operand = false;
    return 0;
}

fn reduce(work: &mut Work, arena: &mut crate::arena::Arena) -> i32 {
    let operator_count = work.operator_count;
    if operator_count < 1 { return fail(&mut *work, 1, 0); }
    let top = operator_count - 1;
    let position = work.positions[top];
    let kind = work.operators[top];
    let operand_count = work.operand_count;
    if kind == 0 || operand_count < 2 { return fail(&mut *work, 1, position); }
    let left_slot = operand_count - 2;
    let right_slot = operand_count - 1;
    let left = work.operands[left_slot];
    let right = work.operands[right_slot];
    let node = crate::arena::append(&mut *arena, kind, 0, left, right);
    if node < 0 { return fail(&mut *work, 6, position); }
    // Replacing two existing roots with one cannot grow the operand stack.
    work.operands[left_slot] = node;
    work.operand_count = operand_count - 1;
    work.operator_count = top;
    return 0;
}

fn binary(work: &mut Work, arena: &mut crate::arena::Arena, kind: i32, position: i32) -> i32 {
    if work.expect_operand { return fail(&mut *work, 1, position); }
    while work.operator_count > 0 {
        let top = work.operator_count - 1;
        if work.operators[top] < kind { break; }
        let status = reduce(&mut *work, &mut *arena);
        if status != 0 { return status; }
    }
    let status = push_operator(&mut *work, kind, position);
    if status != 0 { return status; }
    work.expect_operand = true;
    return 0;
}

fn open(work: &mut Work, position: i32) -> i32 {
    if !work.expect_operand { return fail(&mut *work, 2, position); }
    return push_operator(&mut *work, 0, position);
}

fn innermost_open(work: &Work) -> i32 {
    let mut index = work.operator_count;
    while index > 0 {
        index = index - 1;
        if work.operators[index] == 0 { return index; }
    }
    return -1;
}

fn close(work: &mut Work, arena: &mut crate::arena::Arena, position: i32) -> i32 {
    let marker = innermost_open(&*work);
    if marker < 0 { return fail(&mut *work, 3, position); }
    if work.expect_operand { return fail(&mut *work, 1, position); }
    while work.operator_count > marker + 1 {
        let status = reduce(&mut *work, &mut *arena);
        if status != 0 { return status; }
    }
    work.operator_count = marker;
    return 0;
}

fn finish(work: &mut Work, arena: &mut crate::arena::Arena, position: i32) -> i32 {
    if work.expect_operand { return fail(&mut *work, 1, position); }
    let marker = innermost_open(&*work);
    if marker >= 0 {
        let opening_position = work.positions[marker];
        return fail(&mut *work, 4, opening_position);
    }
    while work.operator_count > 0 {
        let status = reduce(&mut *work, &mut *arena);
        if status != 0 { return status; }
    }
    if work.operand_count != 1 { return fail(&mut *work, 1, position); }
    return 0;
}

pub fn parse(codes: &[i32], arena: &mut crate::arena::Arena) -> ParseResult {
    arena.count = 0;
    if codes.len() > 128 { return ParseResult::InputLimit(128); }
    let mut work = new_work();
    let mut cursor = crate::scanner::new_cursor();
    let mut done = false;
    while !done {
        let token = crate::scanner::next_token(&*codes, &mut cursor);
        let position = crate::scanner::token_start(&cursor);
        let mut status = 0;
        match token {
            crate::scanner::Token::Integer(value) => {
                status = literal(&mut work, &mut *arena, value, position);
            },
            crate::scanner::Token::Plus => {
                status = binary(&mut work, &mut *arena, 2, position);
            },
            crate::scanner::Token::Star => {
                status = binary(&mut work, &mut *arena, 3, position);
            },
            crate::scanner::Token::LeftParen => {
                status = open(&mut work, position);
            },
            crate::scanner::Token::RightParen => {
                status = close(&mut work, &mut *arena, position);
            },
            crate::scanner::Token::End => {
                status = finish(&mut work, &mut *arena, position);
                done = true;
            },
            crate::scanner::Token::Invalid(code) => {
                return ParseResult::InvalidCharacter(position);
            },
        }
        if status != 0 { return failure(status, work.error_position); }
    }
    return ParseResult::Parsed(work.operands[0]);
}
