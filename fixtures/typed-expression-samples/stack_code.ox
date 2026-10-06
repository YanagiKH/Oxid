pub struct Code {
    pub opcode: [i32; 15],
    pub operand: [i32; 15],
    pub count: i32,
}

pub fn new_code() -> Code {
    return Code {
        opcode: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        operand: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        count: 0,
    };
}

pub enum CodeResult {
    Value(i32),
    InvalidCode(i32),
}

// Validate every used row before executing any expression arithmetic.
// -2 succeeds, -1 rejects count, a row index identifies the first bad row,
// and count identifies a prefix that leaves more than one stack value.
fn validate(code: &Code) -> i32 {
    if code.count < 1 || code.count > 15 { return -1; }
    let mut height = 0;
    let mut index = 0;
    while index < code.count {
        let opcode = code.opcode[index];
        let operand = code.operand[index];
        if opcode == 1 {
            if operand < 0 { return index; }
            height = height + 1;
        } else {
            if opcode != 2 && opcode != 3 { return index; }
            if operand != 0 || height < 2 { return index; }
            height = height - 1;
        }
        index = index + 1;
    }
    if height != 1 { return code.count; }
    return -2;
}

pub fn execute(code: &Code) -> CodeResult {
    let invalid = validate(&*code);
    if invalid != -2 { return CodeResult::InvalidCode(invalid); }
    // The same immutable borrow covers validation and execution. A valid
    // prefix fits this stack and never reads the unused instruction tail.
    let mut stack = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut height = 0;
    let mut index = 0;
    while index < code.count {
        let opcode = code.opcode[index];
        if opcode == 1 {
            stack[height] = code.operand[index];
            height = height + 1;
        } else {
            let left = stack[height - 2];
            let right = stack[height - 1];
            if opcode == 2 {
                stack[height - 2] = left + right;
            } else {
                stack[height - 2] = left * right;
            }
            height = height - 1;
        }
        index = index + 1;
    }
    return CodeResult::Value(stack[0]);
}
