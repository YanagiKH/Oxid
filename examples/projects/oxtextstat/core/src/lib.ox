use stream::Decoder;
use stream::Step;
use stream::Finish;
use stream::push_byte;
use stream::finish;
use decimal::FormatResult;
use decimal::format_i32;

// Persistent oxtextstat statistics shared by the Process read loop and the
// pure chunk-partition contracts. No I/O is imported here.
// state: 0 open, 1 finalized, 65 malformed/incomplete UTF-8, 70 overflow or
// unexpected decoder lifecycle result. Failures are sticky.
pub struct Counts {
    bytes: i32,
    scalars: i32,
    lines: i32,
    last_lf: bool,
    state: i32
}

pub fn new_counts() -> Counts {
    return Counts { bytes: 0, scalars: 0, lines: 0, last_lf: false, state: 0 };
}

fn fail(counts: &mut Counts, status: i32) -> i32 {
    counts.state = status;
    return status;
}

// Feeds every byte of one logical chunk exactly once, in order, into the
// retained decoder. A chunk boundary never finalizes the stream.
// Returns 0 when every byte was accepted, otherwise the sticky status.
pub fn feed(decoder: &mut Decoder, counts: &mut Counts, bytes: &[u8]) -> i32 {
    if counts.state == 1 {
        return 70;
    }
    if counts.state != 0 {
        return counts.state;
    }
    let length = bytes.len();
    let mut index = 0;
    while index < length {
        let current = bytes[index];
        let step = push_byte(&mut *decoder, current);
        match step {
            Step::Pending => {
                if counts.bytes == 2147483647 {
                    return fail(&mut *counts, 70);
                }
                counts.bytes = counts.bytes + 1;
            },
            Step::Scalar(scalar) => {
                if counts.bytes == 2147483647 {
                    return fail(&mut *counts, 70);
                }
                counts.bytes = counts.bytes + 1;
                if counts.scalars == 2147483647 {
                    return fail(&mut *counts, 70);
                }
                counts.scalars = counts.scalars + 1;
                if scalar == 10 {
                    if counts.lines == 2147483647 {
                        return fail(&mut *counts, 70);
                    }
                    counts.lines = counts.lines + 1;
                    counts.last_lf = true;
                } else {
                    counts.last_lf = false;
                }
            },
            Step::Invalid(offset) => {
                return fail(&mut *counts, 65);
            },
            Step::PositionOverflow => {
                return fail(&mut *counts, 70);
            },
            Step::Finished => {
                return fail(&mut *counts, 70);
            },
        }
        index = index + 1;
    }
    return 0;
}

// Call only at real EOF. Applies the checked unterminated-line adjustment
// once. A second call is a lifecycle error (70) and changes nothing.
pub fn finalize(decoder: &mut Decoder, counts: &mut Counts) -> i32 {
    if counts.state == 1 {
        return 70;
    }
    if counts.state != 0 {
        return counts.state;
    }
    let finished = finish(&mut *decoder);
    match finished {
        Finish::Complete => {},
        Finish::Invalid(offset) => {
            return fail(&mut *counts, 65);
        },
        Finish::PositionOverflow => {
            return fail(&mut *counts, 70);
        },
    }
    if counts.bytes > 0 && !counts.last_lf {
        if counts.lines == 2147483647 {
            return fail(&mut *counts, 70);
        }
        counts.lines = counts.lines + 1;
    }
    counts.state = 1;
    return 0;
}

// Copies one canonical i32_decimal field into output at start.
// Returns the next position, or -2 when output is too small.
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

// Writes `<bytes> <scalars> <lines>\n` into the output prefix and returns its
// length. Returns -1 before successful finalization and -2 when too small.
pub fn render(counts: &Counts, output: &mut [u8]) -> i32 {
    if counts.state != 1 {
        return -1;
    }
    let first = put_decimal(counts.bytes, &mut *output, 0);
    if first < 0 {
        return first;
    }
    let first_space = put_byte(32, &mut *output, first);
    if first_space < 0 {
        return first_space;
    }
    let second = put_decimal(counts.scalars, &mut *output, first_space);
    if second < 0 {
        return second;
    }
    let second_space = put_byte(32, &mut *output, second);
    if second_space < 0 {
        return second_space;
    }
    let third = put_decimal(counts.lines, &mut *output, second_space);
    if third < 0 {
        return third;
    }
    return put_byte(10, &mut *output, third);
}
