use crate::stream::Decoder;
use crate::stream::End;
use crate::stream::Read;
use crate::stream::finish;
use crate::stream::new_decoder;
use crate::stream::next;

fn consume(decoder: &mut Decoder, chunk: &[u8], already_read: i32) -> i32 {
    let mut cursor = 0;
    let mut count = already_read;
    while true {
        let decoded = next(&mut *decoder, &*chunk, cursor);
        match decoded {
            Read::NeedInput => { return count; },
            Read::Invalid(offset) => { return -1; },
            Read::TooLong => { return -2; },
            Read::InvalidCursor => { return -3; },
            Read::Character(character) => {
                let value = character.value;
                let offset = character.byte_offset;
                if count == 0 && (value.to_u32() != 0u32 || offset != 0i64) { return -4; }
                if count == 1 && (value.to_u32() != 128u32 || offset != 1i64) { return -5; }
                if count == 2 && (value.to_u32() != 2048u32 || offset != 3i64) { return -6; }
                if count == 3 && (value.to_u32() != 65536u32 || offset != 6i64) { return -7; }
                if count == 4 && (value.to_u32() != 1114111u32 || offset != 10i64) { return -8; }
                if count >= 5 || character.next_in_chunk <= cursor { return -9; }
                count = count + 1;
                cursor = character.next_in_chunk;
            },
        }
    }
    return -10;
}

fn invalid_sequence(input: &[u8]) -> bool {
    let mut decoder = new_decoder();
    let prefix = b"A\xF0";
    let first = next(&mut decoder, &prefix, 0);
    match first {
        Read::Character(character) => {
            if character.next_in_chunk != 1 || character.byte_offset != 0i64 || character.value != 'A' {
                return false;
            }
        },
        Read::NeedInput => { return false; },
        Read::Invalid(offset) => { return false; },
        Read::TooLong => { return false; },
        Read::InvalidCursor => { return false; },
    }
    let partial = next(&mut decoder, &prefix, 1);
    match partial {
        Read::NeedInput => {},
        Read::Character(character) => { return false; },
        Read::Invalid(offset) => { return false; },
        Read::TooLong => { return false; },
        Read::InvalidCursor => { return false; },
    }
    let bad = next(&mut decoder, &*input, 0);
    match bad {
        Read::Invalid(offset) => { if offset != 1i64 { return false; } },
        Read::Character(character) => { return false; },
        Read::NeedInput => { return false; },
        Read::TooLong => { return false; },
        Read::InvalidCursor => { return false; },
    }
    let end = finish(&decoder);
    match end {
        End::Invalid(offset) => { return offset == 1i64; },
        End::Complete => { return false; },
        End::Truncated(offset) => { return false; },
        End::TooLong => { return false; },
    }
}

fn truncated_and_retry() -> bool {
    let lead = b"\xE9";
    let rest = b"\x9B\xAA";
    let mut decoder = new_decoder();
    let bad_cursor = next(&mut decoder, &lead, 2);
    match bad_cursor {
        Read::InvalidCursor => {},
        Read::Character(character) => { return false; },
        Read::NeedInput => { return false; },
        Read::Invalid(offset) => { return false; },
        Read::TooLong => { return false; },
    }
    let partial = next(&mut decoder, &lead, 0);
    match partial {
        Read::NeedInput => {},
        Read::Character(character) => { return false; },
        Read::Invalid(offset) => { return false; },
        Read::TooLong => { return false; },
        Read::InvalidCursor => { return false; },
    }
    let end = finish(&decoder);
    match end {
        End::Complete => { return false; },
        End::Invalid(offset) => { return false; },
        End::Truncated(offset) => { if offset != 0i64 { return false; } },
        End::TooLong => { return false; },
    }
    let decoded = next(&mut decoder, &rest, 0);
    match decoded {
        Read::Character(character) => {
            return character.value == '雪' && character.next_in_chunk == 2 && character.byte_offset == 0i64;
        },
        Read::NeedInput => { return false; },
        Read::Invalid(offset) => { return false; },
        Read::TooLong => { return false; },
        Read::InvalidCursor => { return false; },
    }
}

fn main() -> i32 {
    let data = b"\x00\xC2\x80\xE0\xA0\x80\xF0\x90\x80\x80\xF4\x8F\xBF\xBF";
    let mut split = 0;
    while split <= 14 {
        let mut decoder = new_decoder();
        let first = consume(&mut decoder, &data[0..split], 0);
        if first < 0 { return 1; }
        let complete = consume(&mut decoder, &data[split..14], first);
        if complete != 5 { return 2; }
        let end = finish(&decoder);
        match end {
            End::Complete => {},
            End::Invalid(offset) => { return 3; },
            End::Truncated(offset) => { return 4; },
            End::TooLong => { return 5; },
        }
        split = split + 1;
    }
    let overlong = b"\x8F\x80\x80";
    let interrupted = b"\x90X\x80";
    if !invalid_sequence(&overlong) { return 6; }
    if !invalid_sequence(&interrupted) { return 7; }
    if !truncated_and_retry() { return 8; }
    return 0;
}
