pub struct Decoder {
    offset: i64,
    sequence_start: i64,
    scalar: i32,
    remaining: i32,
    minimum: i32,
    maximum: i32,
    invalid: bool,
    too_long: bool,
}

pub struct Decoded {
    pub value: char,
    pub byte_offset: i64,
    pub next_in_chunk: i32,
}

pub enum Read {
    Character(Decoded),
    NeedInput,
    Invalid(i64),
    TooLong,
    InvalidCursor,
}

pub enum End {
    Complete,
    Truncated(i64),
    Invalid(i64),
    TooLong,
}

pub fn new_decoder() -> Decoder {
    return Decoder {
        offset: 0i64, sequence_start: 0i64, scalar: 0, remaining: 0,
        minimum: 128, maximum: 191, invalid: false, too_long: false,
    };
}

fn begin(decoder: &mut Decoder, lead: i32) -> bool {
    if lead <= 127 {
        decoder.scalar = lead;
        return true;
    }
    if lead < 194 || lead > 244 {
        return false;
    }
    if lead <= 223 {
        decoder.scalar = lead - 192;
        decoder.remaining = 1;
    } else {
        if lead <= 239 {
            decoder.scalar = lead - 224;
            decoder.remaining = 2;
        } else {
            decoder.scalar = lead - 240;
            decoder.remaining = 3;
        }
    }

    // Limit the second byte to reject surrogate code points, overlong
    // encodings and code points beyond U+10FFFF as soon as they are known.
    decoder.minimum = 128;
    decoder.maximum = 191;
    if lead == 224 { decoder.minimum = 160; }
    if lead == 237 { decoder.maximum = 159; }
    if lead == 240 { decoder.minimum = 144; }
    if lead == 244 { decoder.maximum = 143; }
    return true;
}

fn extend(decoder: &mut Decoder, byte: i32) -> bool {
    if byte < decoder.minimum || byte > decoder.maximum {
        return false;
    }
    decoder.scalar = decoder.scalar * 64 + (byte - 128);
    decoder.remaining = decoder.remaining - 1;
    decoder.minimum = 128;
    decoder.maximum = 191;
    return true;
}

pub fn next(decoder: &mut Decoder, input: &[u8], start: i32) -> Read {
    if decoder.invalid { return Read::Invalid(decoder.sequence_start); }
    if decoder.too_long { return Read::TooLong; }
    if start < 0 || start > input.len() { return Read::InvalidCursor; }

    let mut cursor = start;
    while cursor < input.len() {
        let offset = decoder.offset;
        if offset == 9223372036854775807i64 {
            decoder.too_long = true;
            return Read::TooLong;
        }
        let byte = input[cursor];
        let value = byte.to_i32();
        let mut valid = false;
        if decoder.remaining == 0 {
            decoder.sequence_start = offset;
            valid = begin(&mut *decoder, value);
        } else {
            valid = extend(&mut *decoder, value);
        }
        if !valid {
            decoder.invalid = true;
            return Read::Invalid(decoder.sequence_start);
        }
        cursor = cursor + 1;
        decoder.offset = offset + 1i64;
        if decoder.remaining == 0 {
            let scalar = decoder.scalar;
            return Read::Character(Decoded {
                value: scalar.to_char_checked(), byte_offset: decoder.sequence_start,
                next_in_chunk: cursor,
            });
        }
    }
    return Read::NeedInput;
}

pub fn finish(decoder: &Decoder) -> End {
    if decoder.invalid { return End::Invalid(decoder.sequence_start); }
    if decoder.too_long { return End::TooLong; }
    if decoder.remaining != 0 { return End::Truncated(decoder.sequence_start); }
    return End::Complete;
}
