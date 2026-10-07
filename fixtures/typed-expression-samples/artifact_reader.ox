pub enum LoadResult {
    Value(i32),
    InvalidArtifact(i32),
}

// -2 succeeds; -1 rejects length; otherwise the first detected bad byte
// offset is returned. A residual operand stack identifies count byte 4.
// No expression arithmetic or decoded Code storage is used in this pass.
fn validate(bytes: &[i32], used: i32) -> i32 {
    if used != 80 || used > bytes.len() { return -1; }
    let mut position = 0;
    while position < 80 {
        if bytes[position] < 0 || bytes[position] > 255 { return position; }
        position = position + 1;
    }
    if bytes[0] != 79 { return 0; }
    if bytes[1] != 88 { return 1; }
    if bytes[2] != 83 { return 2; }
    if bytes[3] != 49 { return 3; }
    let count = bytes[4];
    if count < 1 || count > 15 { return 4; }
    let mut row = 0;
    let mut height = 0;
    while row < count {
        let start = 5 + row * 5;
        let opcode = bytes[start];
        if opcode != 1 && opcode != 2 && opcode != 3 { return start; }
        if bytes[start + 4] > 127 { return start + 4; }
        if opcode == 1 {
            height = height + 1;
        } else {
            position = start + 1;
            while position < start + 5 {
                if bytes[position] != 0 { return position; }
                position = position + 1;
            }
            if height < 2 { return start; }
            height = height - 1;
        }
        row = row + 1;
    }
    if height != 1 { return 4; }
    position = 5 + count * 5;
    while position < 80 {
        if bytes[position] != 0 { return position; }
        position = position + 1;
    }
    return -2;
}

fn operand(bytes: &[i32], start: i32) -> i32 {
    // Validation has bounded the high byte to 127 and the others to 255.
    // Each intermediate is nonnegative and at most i32::MAX.
    let upper = bytes[start + 4] * 256 + bytes[start + 3];
    let middle = upper * 256 + bytes[start + 2];
    return middle * 256 + bytes[start + 1];
}

pub fn load(bytes: &[i32], used: i32) -> LoadResult {
    let invalid = validate(&*bytes, used);
    if invalid != -2 { return LoadResult::InvalidArtifact(invalid); }
    // One shared loan covers every validation and execution read. The loader
    // imports no producer module and consumes only the saved artifact bytes.
    let mut values = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut height = 0;
    let mut row = 0;
    while row < bytes[4] {
        let start = 5 + row * 5;
        let opcode = bytes[start];
        if opcode == 1 {
            values[height] = operand(&*bytes, start);
            height = height + 1;
        } else {
            let left = values[height - 2];
            let right = values[height - 1];
            if opcode == 2 {
                values[height - 2] = left + right;
            } else {
                values[height - 2] = left * right;
            }
            height = height - 1;
        }
        row = row + 1;
    }
    return LoadResult::Value(values[0]);
}
