use crate::stream::End;
use crate::stream::Read;
use crate::stream::finish;
use crate::stream::new_decoder;
use crate::stream::next;

fn main() -> i32 {
    let mut decoder = new_decoder();
    let first_chunk = b"\xE9";
    let second_chunk = b"\x9B\xAA";

    let need_more = next(&mut decoder, &first_chunk, 0);
    match need_more {
        Read::NeedInput => {},
        Read::Character(other) => { return 1; },
        Read::Invalid(offset) => { return 1; },
        Read::TooLong => { return 1; },
        Read::InvalidCursor => { return 1; },
    }
    let decoded = next(&mut decoder, &second_chunk, 0);
    match decoded {
        Read::Character(character) => {
            if character.value != '雪' { return 2; }
        },
        Read::NeedInput => { return 3; },
        Read::Invalid(offset) => { return 3; },
        Read::TooLong => { return 3; },
        Read::InvalidCursor => { return 3; },
    }
    let end = finish(&decoder);
    match end {
        End::Complete => { return 42; },
        End::Truncated(offset) => { return 4; },
        End::Invalid(offset) => { return 4; },
        End::TooLong => { return 4; },
    }
}
