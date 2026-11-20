use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;
use alloc::vec::truncate;
use crate::text::Validation as Utf8Validation;
use crate::text::validate as validate_utf8;
use crate::text::Text;
use crate::text::AppendStatus;
use crate::text::new_text;
use crate::text::append_utf8;
use crate::text::append_char;

pub enum Validation {
    Valid,
    Syntax(i32),
    InvalidUtf8(i32),
    CapacityOverflow,
    AllocationFailed,
    MemoryLimit,
}

pub enum StringResult { Value(Text), Error(Validation) }

enum WriteOutcome { Written, Failed(Validation) }

struct Cursor { offset: i32 }

fn peek(input: &[u8], offset: i32) -> char {
    // This sentinel is outside the range of any input byte.
    if offset == input.len() { return '\u{100}'; }
    let value = input[offset];
    return value.to_char_checked();
}

fn take(input: &[u8], cursor: &mut Cursor, expected: char) -> bool {
    if peek(&*input, cursor.offset) != expected { return false; }
    cursor.offset = cursor.offset + 1;
    return true;
}

fn skip_space(input: &[u8], cursor: &mut Cursor) -> () {
    while cursor.offset < input.len() {
        let byte = peek(&*input, cursor.offset);
        if byte != ' ' && byte != '\t' && byte != '\n' && byte != '\r' { break; }
        cursor.offset = cursor.offset + 1;
    }
    return;
}

fn hex_digit(byte: char) -> i32 {
    let code = byte.to_u32();
    let ordinal = code.to_i32_checked();
    if byte >= '0' && byte <= '9' { return ordinal - 48; }
    if byte >= 'A' && byte <= 'F' { return ordinal - 55; }
    if byte >= 'a' && byte <= 'f' { return ordinal - 87; }
    return -1;
}

fn hex_quad(input: &[u8], cursor: &mut Cursor) -> i32 {
    let mut value = 0;
    for index in 0..4 {
        let byte = peek(&*input, cursor.offset);
        let decoded = hex_digit(byte);
        if decoded < 0 { return -1; }
        value = value * 16 + decoded;
        cursor.offset = cursor.offset + 1;
    }
    return value;
}

fn unicode_escape(input: &[u8], cursor: &mut Cursor) -> i32 {
    let start = cursor.offset;
    let first = hex_quad(&*input, &mut *cursor);
    if first < 55296 || first > 57343 { return first; }
    if first >= 56320 { cursor.offset = start; return -1; }
    if !take(&*input, &mut *cursor, '\\') || !take(&*input, &mut *cursor, 'u') {
        return -1;
    }
    let low_start = cursor.offset;
    let second = hex_quad(&*input, &mut *cursor);
    if second < 0 { return -1; }
    if second < 56320 || second > 57343 { cursor.offset = low_start; return -1; }
    return 65536 + (first - 55296) * 1024 + (second - 56320);
}

fn escape(input: &[u8], cursor: &mut Cursor) -> i32 {
    let byte = peek(&*input, cursor.offset);
    if byte == 'u' {
        cursor.offset = cursor.offset + 1;
        return unicode_escape(&*input, &mut *cursor);
    }
    let mut decoded = -1;
    if byte == '"' || byte == '\\' || byte == '/' {
        let ordinal = byte.to_u32();
        decoded = ordinal.to_i32_checked();
    }
    if byte == 'b' { decoded = 8; }
    if byte == 'f' { decoded = 12; }
    if byte == 'n' { decoded = 10; }
    if byte == 'r' { decoded = 13; }
    if byte == 't' { decoded = 9; }
    if decoded >= 0 { cursor.offset = cursor.offset + 1; }
    return decoded;
}

fn string(input: &[u8], cursor: &mut Cursor) -> bool {
    if !take(&*input, &mut *cursor, '"') { return false; }
    while cursor.offset < input.len() {
        let byte = peek(&*input, cursor.offset);
        if byte < ' ' { return false; }
        cursor.offset = cursor.offset + 1;
        if byte == '"' { return true; }
        if byte == '\\' && escape(&*input, &mut *cursor) < 0 { return false; }
    }
    return false;
}

fn digit(byte: char) -> bool { return byte >= '0' && byte <= '9'; }

fn digits(input: &[u8], cursor: &mut Cursor) -> bool {
    let start = cursor.offset;
    while digit(peek(&*input, cursor.offset)) { cursor.offset = cursor.offset + 1; }
    return cursor.offset > start;
}

fn number(input: &[u8], cursor: &mut Cursor) -> bool {
    take(&*input, &mut *cursor, '-');
    if !take(&*input, &mut *cursor, '0') && !digits(&*input, &mut *cursor) { return false; }
    if take(&*input, &mut *cursor, '.') && !digits(&*input, &mut *cursor) { return false; }
    if take(&*input, &mut *cursor, 'e') || take(&*input, &mut *cursor, 'E') {
        if !take(&*input, &mut *cursor, '+') { take(&*input, &mut *cursor, '-'); }
        return digits(&*input, &mut *cursor);
    }
    return true;
}

fn literal(input: &[u8], cursor: &mut Cursor, expected: &[u8]) -> bool {
    for index in 0..expected.len() {
        let byte = expected[index];
        if !take(&*input, &mut *cursor, byte.to_char_checked()) { return false; }
    }
    return true;
}

fn scalar(input: &[u8], cursor: &mut Cursor) -> bool {
    let byte = peek(&*input, cursor.offset);
    if byte == '"' { return string(&*input, &mut *cursor); }
    if byte == '-' || digit(byte) { return number(&*input, &mut *cursor); }
    if byte == 't' { let truth = b"true"; return literal(&*input, &mut *cursor, &truth); }
    if byte == 'f' { let falsity = b"false"; return literal(&*input, &mut *cursor, &falsity); }
    if byte == 'n' { let nil = b"null"; return literal(&*input, &mut *cursor, &nil); }
    return false;
}

fn member_name(input: &[u8], cursor: &mut Cursor) -> bool {
    skip_space(&*input, &mut *cursor);
    if !string(&*input, &mut *cursor) { return false; }
    skip_space(&*input, &mut *cursor);
    return take(&*input, &mut *cursor, ':');
}

pub fn validate(input: &[u8]) -> Validation {
    let encoding = validate_utf8(&*input);
    match encoding {
        Utf8Validation::Valid => {},
        Utf8Validation::Invalid(offset) => { return Validation::InvalidUtf8(offset); },
    }
    let mut cursor = Cursor { offset: 0 };
    let mut closings = new::<char>();
    let mut needs_value = true;
    while true {
        skip_space(&*input, &mut cursor);
        if needs_value {
            let marker = peek(&*input, cursor.offset);
            if marker == '[' || marker == '{' {
                cursor.offset = cursor.offset + 1;
                skip_space(&*input, &mut cursor);
                let mut closing = ']';
                if marker == '{' { closing = '}'; }
                if take(&*input, &mut cursor, closing) { needs_value = false; continue; }
                if closing == '}' && !member_name(&*input, &mut cursor) {
                    return Validation::Syntax(cursor.offset);
                }
                let added = try_push(&mut closings, closing);
                match added {
                    GrowStatus::Ok => {},
                    GrowStatus::CapacityOverflow => { return Validation::CapacityOverflow; },
                    GrowStatus::AllocationFailed => { return Validation::AllocationFailed; },
                    GrowStatus::MemoryLimit => { return Validation::MemoryLimit; },
                }
                continue;
            }
            if !scalar(&*input, &mut cursor) { return Validation::Syntax(cursor.offset); }
            needs_value = false;
            continue;
        }
        let depth = len(&closings);
        if depth == 0 {
            if cursor.offset == input.len() { return Validation::Valid; }
            return Validation::Syntax(cursor.offset);
        }
        let closing = closings[depth - 1];
        if take(&*input, &mut cursor, closing) {
            truncate(&mut closings, depth - 1);
            continue;
        }
        if !take(&*input, &mut cursor, ',') { return Validation::Syntax(cursor.offset); }
        if closing == '}' && !member_name(&*input, &mut cursor) {
            return Validation::Syntax(cursor.offset);
        }
        needs_value = true;
    }
    return Validation::Syntax(cursor.offset);
}

fn write_outcome(status: AppendStatus, start: i32) -> WriteOutcome {
    match status {
        AppendStatus::Ok => { return WriteOutcome::Written; },
        AppendStatus::InvalidInput(offset) => {
            return WriteOutcome::Failed(Validation::InvalidUtf8(start + offset));
        },
        AppendStatus::CapacityOverflow => { return WriteOutcome::Failed(Validation::CapacityOverflow); },
        AppendStatus::AllocationFailed => { return WriteOutcome::Failed(Validation::AllocationFailed); },
        AppendStatus::MemoryLimit => { return WriteOutcome::Failed(Validation::MemoryLimit); },
    }
}

fn write_run(output: &mut Text, input: &[u8], start: i32, end: i32) -> WriteOutcome {
    if start == end { return WriteOutcome::Written; }
    return write_outcome(append_utf8(&mut *output, &*input[start..end]), start);
}

pub fn decode_string(input: &[u8]) -> StringResult {
    let encoding = validate_utf8(&*input);
    match encoding {
        Utf8Validation::Valid => {},
        Utf8Validation::Invalid(offset) => { return StringResult::Error(Validation::InvalidUtf8(offset)); },
    }
    let mut cursor = Cursor { offset: 0 };
    skip_space(&*input, &mut cursor);
    let opening = cursor.offset;
    if !string(&*input, &mut cursor) { return StringResult::Error(Validation::Syntax(cursor.offset)); }
    let closing = cursor.offset - 1;
    skip_space(&*input, &mut cursor);
    if cursor.offset != input.len() { return StringResult::Error(Validation::Syntax(cursor.offset)); }

    let mut output = new_text();
    let mut start = opening + 1;
    cursor.offset = start;
    while cursor.offset < closing {
        let index = cursor.offset;
        cursor.offset = cursor.offset + 1;
        if peek(&*input, index) != '\\' { continue; }
        let wrote_run = write_run(&mut output, &*input, start, index);
        match wrote_run {
            WriteOutcome::Written => {},
            WriteOutcome::Failed(error) => { return StringResult::Error(error); },
        }
        let decoded = escape(&*input, &mut cursor);
        if decoded < 0 { return StringResult::Error(Validation::Syntax(cursor.offset)); }
        let wrote_escape = write_outcome(append_char(&mut output, decoded.to_char_checked()), index);
        match wrote_escape {
            WriteOutcome::Written => {},
            WriteOutcome::Failed(error) => { return StringResult::Error(error); },
        }
        start = cursor.offset;
    }
    let wrote_tail = write_run(&mut output, &*input, start, closing);
    match wrote_tail {
        WriteOutcome::Written => { return StringResult::Value(output); },
        WriteOutcome::Failed(error) => { return StringResult::Error(error); },
    }
}

fn write_quoted_escape(output: &mut Text, byte: u8, position: i32) -> WriteOutcome {
    let ordinal = byte.to_i32();
    if ordinal >= 32 {
        let mut pair = b"\\.";
        pair[1] = byte;
        return write_outcome(append_utf8(&mut *output, &pair), position);
    }
    let hex = b"0123456789abcdef";
    let mut escaped = b"\\u0000";
    escaped[4] = hex[ordinal / 16];
    escaped[5] = hex[ordinal % 16];
    return write_outcome(append_utf8(&mut *output, &escaped), position);
}

pub fn quote(input: &[u8]) -> StringResult {
    let encoding = validate_utf8(&*input);
    match encoding {
        Utf8Validation::Valid => {},
        Utf8Validation::Invalid(offset) => { return StringResult::Error(Validation::InvalidUtf8(offset)); },
    }
    let mut output = new_text();
    let started = write_outcome(append_char(&mut output, '"'), 0);
    match started {
        WriteOutcome::Written => {},
        WriteOutcome::Failed(error) => { return StringResult::Error(error); },
    }
    let mut start = 0;
    for index in 0..input.len() {
        let byte = peek(&*input, index);
        if byte >= ' ' && byte != '"' && byte != '\\' { continue; }
        let wrote_run = write_run(&mut output, &*input, start, index);
        match wrote_run {
            WriteOutcome::Written => {},
            WriteOutcome::Failed(error) => { return StringResult::Error(error); },
        }
        let wrote_escape = write_quoted_escape(&mut output, input[index], index);
        match wrote_escape {
            WriteOutcome::Written => {},
            WriteOutcome::Failed(error) => { return StringResult::Error(error); },
        }
        start = index + 1;
    }
    let wrote_tail = write_run(&mut output, &*input, start, input.len());
    match wrote_tail {
        WriteOutcome::Written => {},
        WriteOutcome::Failed(error) => { return StringResult::Error(error); },
    }
    let finished = write_outcome(append_char(&mut output, '"'), input.len());
    match finished {
        WriteOutcome::Written => { return StringResult::Value(output); },
        WriteOutcome::Failed(error) => { return StringResult::Error(error); },
    }
}
