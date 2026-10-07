use std::io::write_stdout;
use std::io::WriteStatus;

// This validates the complete used Code prefix without doing expression
// arithmetic. Unused Code rows are deliberately never inspected.
fn validate(code: &crate::stack_code::Code) -> bool {
    if code.count < 1 || code.count > 15 { return false; }
    let mut height = 0;
    let mut index = 0;
    while index < code.count {
        let opcode = code.opcode[index];
        let operand = code.operand[index];
        if opcode == 1 {
            if operand < 0 { return false; }
            height = height + 1;
        } else {
            if opcode != 2 && opcode != 3 { return false; }
            if operand != 0 || height < 2 { return false; }
            height = height - 1;
        }
        index = index + 1;
    }
    return height == 1;
}

fn encode_row(bytes: &mut [i32], start: i32, opcode: i32, operand: i32) -> () {
    bytes[start] = opcode;
    let mut remaining = operand;
    bytes[start + 1] = remaining % 256;
    remaining = remaining / 256;
    bytes[start + 2] = remaining % 256;
    remaining = remaining / 256;
    bytes[start + 3] = remaining % 256;
    remaining = remaining / 256;
    bytes[start + 4] = remaining;
    return ();
}

// Process statuses: 0 complete, 65 invalid Code, 70 impossible byte domain,
// 74 output I/O error. Ordinary source failures keep the runtime's status.
pub fn emit(code: &crate::stack_code::Code) -> i32 {
    if !validate(&*code) { return 65; }
    let mut bytes = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    bytes[0] = 79;
    bytes[1] = 88;
    bytes[2] = 83;
    bytes[3] = 49;
    bytes[4] = code.count;
    let mut index = 0;
    while index < code.count {
        encode_row(&mut bytes, 5 + index * 5, code.opcode[index], code.operand[index]);
        index = index + 1;
    }
    // The same immutable Code loan covers validation and serialization.
    // Fresh zero initialization supplies every unused artifact row.
    let result = write_stdout(&bytes);
    match result {
        WriteStatus::Complete => { return 0; },
        WriteStatus::InvalidInput => { return 70; },
        WriteStatus::IoError(accepted) => { return 74; },
    }
}
