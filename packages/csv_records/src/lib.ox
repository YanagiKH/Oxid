use alloc::bytes::ByteBuf;
use alloc::bytes::len as bytes_len;

pub struct Reader {
    bytes: ByteBuf,
    next: i32,
    done: bool,
    failed: bool,
    error_offset: i32,
    current: bool,
    start: i32,
    end: i32,
    decoded_len: i32,
    quoted: bool
}

pub struct FieldInfo {
    pub decoded_len: i32,
    pub record_end: bool
}

pub enum Next {
    Field(FieldInfo),
    End,
    Malformed(i32)
}

pub enum CopyResult {
    Written(i32),
    BufferTooSmall(i32),
    NoCurrent
}

struct Scanned {
    start: i32,
    end: i32,
    decoded_len: i32,
    quoted: bool,
    next: i32,
    record_end: bool,
    done: bool
}

enum ScanResult {
    Field(Scanned),
    Malformed(i32)
}

pub fn new_reader(input: ByteBuf) -> Reader {
    let empty = bytes_len(&input) == 0;
    return Reader {
        bytes: input,
        next: 0,
        done: empty,
        failed: false,
        error_offset: 0,
        current: false,
        start: 0,
        end: 0,
        decoded_len: 0,
        quoted: false
    };
}

// The field body has already been checked. Require a delimiter or EOF.
fn finish_field(input: &[u8], start: i32, end: i32, decoded_len: i32, quoted: bool, after: i32) -> ScanResult {
    let length = input.len();
    if after == length {
        return ScanResult::Field(Scanned { start: start, end: end, decoded_len: decoded_len, quoted: quoted, next: after, record_end: true, done: true });
    }
    let separator_byte = input[after];
    let separator = separator_byte.to_i32();
    if separator == 44 {
        return ScanResult::Field(Scanned { start: start, end: end, decoded_len: decoded_len, quoted: quoted, next: after + 1, record_end: false, done: false });
    }
    if separator == 10 {
        let next = after + 1;
        return ScanResult::Field(Scanned { start: start, end: end, decoded_len: decoded_len, quoted: quoted, next: next, record_end: true, done: next == length });
    }
    if separator == 13 {
        if length - after < 2 {
            return ScanResult::Malformed(after);
        }
        let following_byte = input[after + 1];
        if following_byte.to_i32() != 10 {
            return ScanResult::Malformed(after);
        }
        let next = after + 2;
        return ScanResult::Field(Scanned { start: start, end: end, decoded_len: decoded_len, quoted: quoted, next: next, record_end: true, done: next == length });
    }
    return ScanResult::Malformed(after);
}

fn scan_quoted(input: &[u8], opening: i32) -> ScanResult {
    let length = input.len();
    let start = opening + 1;
    let mut index = start;
    let mut decoded_len = 0;
    while index < length {
        let current_byte = input[index];
        if current_byte.to_i32() == 34 {
            let after = index + 1;
            if after < length {
                let following_byte = input[after];
                if following_byte.to_i32() == 34 {
                    decoded_len = decoded_len + 1;
                    index = after + 1;
                    continue;
                }
            }
            return finish_field(&*input, start, index, decoded_len, true, after);
        }
        decoded_len = decoded_len + 1;
        index = index + 1;
    }
    return ScanResult::Malformed(opening);
}

fn scan_field(input: &[u8], start: i32) -> ScanResult {
    let length = input.len();
    if start < length {
        let first_byte = input[start];
        if first_byte.to_i32() == 34 {
            return scan_quoted(&*input, start);
        }
    }
    let mut index = start;
    while index < length {
        let current_byte = input[index];
        let current = current_byte.to_i32();
        if current == 34 {
            return ScanResult::Malformed(index);
        }
        if current == 44 || current == 10 || current == 13 {
            return finish_field(&*input, start, index, index - start, false, index);
        }
        index = index + 1;
    }
    return finish_field(&*input, start, index, index - start, false, index);
}

pub fn next_field(reader: &mut Reader) -> Next {
    reader.current = false;
    if reader.failed {
        return Next::Malformed(reader.error_offset);
    }
    if reader.done {
        return Next::End;
    }
    let scanned = scan_field(&*reader.bytes, reader.next);
    match scanned {
        ScanResult::Malformed(offset) => {
            reader.failed = true;
            reader.error_offset = offset;
            return Next::Malformed(offset);
        },
        ScanResult::Field(field) => {
            reader.start = field.start;
            reader.end = field.end;
            reader.decoded_len = field.decoded_len;
            reader.quoted = field.quoted;
            reader.next = field.next;
            reader.done = field.done;
            reader.current = true;
            return Next::Field(FieldInfo { decoded_len: field.decoded_len, record_end: field.record_end });
        },
    }
}

// Only called for a checked private current field after destination sizing.
fn copy_decoded(input: &[u8], start: i32, end: i32, quoted: bool, output: &mut [u8]) -> () {
    let mut source = start;
    let mut destination = 0;
    while source < end {
        let current_byte = input[source];
        output[destination] = current_byte;
        destination = destination + 1;
        if quoted && current_byte.to_i32() == 34 {
            source = source + 2;
        } else {
            source = source + 1;
        }
    }
    return;
}

pub fn copy_current_field(reader: &Reader, output: &mut [u8]) -> CopyResult {
    if !reader.current {
        return CopyResult::NoCurrent;
    }
    let required = reader.decoded_len;
    if output.len() < required {
        return CopyResult::BufferTooSmall(required);
    }
    copy_decoded(&*reader.bytes, reader.start, reader.end, reader.quoted, &mut *output);
    return CopyResult::Written(required);
}
