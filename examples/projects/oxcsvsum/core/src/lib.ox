use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use csv::Next;
use csv::CopyResult;
use csv::new_reader;
use csv::next_field;
use csv::copy_current_field;
use text::Validation;
use text::validate;
use decimal::ParseResult;
use decimal::FormatResult;
use decimal::parse_i32;
use decimal::format_i32;

// Complete-buffer two-column CSV numeric summary shared by the oxcsvsum
// Process application and the pure contract project. No I/O is imported here.
// state: 0 not yet summarized, 1 complete; any other value is the sticky
// failure status (65 invalid input, 70 checked overflow, capacity overflow or
// impossible library result, 71 allocation or memory-policy refusal).
pub struct Summary {
    rows: i32,
    total: i32,
    state: i32
}

pub fn new_summary() -> Summary {
    return Summary { rows: 0, total: 0, state: 0 };
}

fn fail(summary: &mut Summary, status: i32) -> i32 {
    summary.state = status;
    return status;
}

fn grow_status(status: GrowStatus) -> i32 {
    match status {
        GrowStatus::Ok => { return 0; },
        GrowStatus::CapacityOverflow => { return 70; },
        GrowStatus::AllocationFailed => { return 71; },
        GrowStatus::MemoryLimit => { return 71; },
    }
}

// Checked row count and signed sum in input record order. The subtraction
// bounds are representable for the selected sign; i32::MIN is never negated.
// Only adds after every check succeeds.
fn add_value(summary: &mut Summary, value: i32) -> i32 {
    if summary.rows == 2147483647 {
        return fail(&mut *summary, 70);
    }
    if value > 0 {
        if summary.total > 2147483647 - value {
            return fail(&mut *summary, 70);
        }
    }
    if value < 0 {
        if summary.total < -2147483648 - value {
            return fail(&mut *summary, 70);
        }
    }
    summary.rows = summary.rows + 1;
    summary.total = summary.total + value;
    return 0;
}

// The complete decoded value field must be one i32_decimal number.
fn add_parsed(summary: &mut Summary, parsed: ParseResult) -> i32 {
    match parsed {
        ParseResult::Value(value) => { return add_value(&mut *summary, value); },
        ParseResult::Empty => { return fail(&mut *summary, 65); },
        ParseResult::SignOnly => { return fail(&mut *summary, 65); },
        ParseResult::InvalidDigit(offset) => { return fail(&mut *summary, 65); },
        ParseResult::Overflow(offset) => { return fail(&mut *summary, 65); },
    }
}

// Consumes the complete input owner. UTF-8 is validated over the whole input
// before ownership moves into the reader; the payload is not copied to do so.
// Each record must be exactly a label field (record_end false, never copied)
// and a value field (record_end true) decoded into one reused growable
// ByteBuf. Returns 0 after End with every record accepted, otherwise the
// sticky status. A second call is a lifecycle error (70).
pub fn summarize(input: ByteBuf, summary: &mut Summary) -> i32 {
    if summary.state != 0 {
        return 70;
    }
    let checked = validate(&input);
    match checked {
        Validation::Valid => {},
        Validation::Invalid(offset) => { return fail(&mut *summary, 65); },
    }
    let mut reader = new_reader(input);
    let mut scratch = new();
    while true {
        let label = next_field(&mut reader);
        match label {
            Next::Field(info) => {
                if info.record_end {
                    return fail(&mut *summary, 65);
                }
            },
            Next::End => {
                summary.state = 1;
                return 0;
            },
            Next::Malformed(offset) => { return fail(&mut *summary, 65); },
        }
        let value = next_field(&mut reader);
        let mut needed = 0;
        match value {
            Next::Field(info) => {
                if !info.record_end {
                    return fail(&mut *summary, 65);
                }
                needed = info.decoded_len;
            },
            Next::End => { return fail(&mut *summary, 70); },
            Next::Malformed(offset) => { return fail(&mut *summary, 65); },
        }
        if len(&scratch) < needed {
            let zero = 0;
            let grown = grow_status(try_resize(&mut scratch, needed, zero.to_u8_checked()));
            if grown != 0 {
                return fail(&mut *summary, grown);
            }
        }
        let copied = copy_current_field(&reader, &mut scratch);
        let mut count = -1;
        match copied {
            CopyResult::Written(written) => { count = written; },
            CopyResult::BufferTooSmall(required) => { return fail(&mut *summary, 70); },
            CopyResult::NoCurrent => { return fail(&mut *summary, 70); },
        }
        if count != needed {
            return fail(&mut *summary, 70);
        }
        let added = add_parsed(&mut *summary, parse_i32(&scratch[0..count]));
        if added != 0 {
            return added;
        }
    }
    return fail(&mut *summary, 70);
}

// Copies one canonical i32_decimal field into output at start through an
// 11-byte scratch array. Returns the next position, or -2 when too small.
fn put_decimal(value: i32, output: &mut [u8], start: i32) -> i32 {
    let mut scratch = b"\0\0\0\0\0\0\0\0\0\0\0";
    let formatted = format_i32(value, &mut scratch);
    match formatted {
        FormatResult::Written(count) => {
            if count > output.len() - start {
                return -2;
            }
            let mut index = 0;
            while index < count {
                let digit = scratch[index];
                output[start + index] = digit;
                index = index + 1;
            }
            return start + count;
        },
        FormatResult::BufferTooSmall(required) => {
            return -2;
        },
    }
}

fn put_byte(value: i32, output: &mut [u8], at: i32) -> i32 {
    if at >= output.len() {
        return -2;
    }
    output[at] = value.to_u8_checked();
    return at + 1;
}

// Writes `<rows> <total>\n` into the output prefix and returns its length.
// Returns -1 unless summarize completed (output unchanged) and -2 when the
// output is too small. At most 24 bytes are needed.
pub fn render(summary: &Summary, output: &mut [u8]) -> i32 {
    if summary.state != 1 {
        return -1;
    }
    let first = put_decimal(summary.rows, &mut *output, 0);
    if first < 0 {
        return first;
    }
    let space = put_byte(32, &mut *output, first);
    if space < 0 {
        return space;
    }
    let second = put_decimal(summary.total, &mut *output, space);
    if second < 0 {
        return second;
    }
    return put_byte(10, &mut *output, second);
}
