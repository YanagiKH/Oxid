pub struct Decoder {
    offset: i32,
    sequence_start: i32,
    scalar: i32,
    remaining: i32,
    minimum: i32,
    maximum: i32,
    state: i32,
    error_offset: i32
}

pub enum Step {
    Pending,
    Scalar(i32),
    Invalid(i32),
    PositionOverflow,
    Finished
}

pub enum Finish {
    Complete,
    Invalid(i32),
    PositionOverflow
}

pub fn new_decoder() -> Decoder {
    return Decoder {
        offset: 0,
        sequence_start: 0,
        scalar: 0,
        remaining: 0,
        minimum: 128,
        maximum: 191,
        state: 0,
        error_offset: 0
    };
}

fn fail(decoder: &mut Decoder, offset: i32) -> Step {
    decoder.state = 1;
    decoder.error_offset = offset;
    return Step::Invalid(offset);
}

fn continue_scalar(decoder: &mut Decoder, value: i32) -> Step {
    if value < decoder.minimum || value > decoder.maximum {
        let start = decoder.sequence_start;
        return fail(&mut *decoder, start);
    }
    decoder.scalar = decoder.scalar * 64 + value - 128;
    decoder.remaining = decoder.remaining - 1;
    decoder.minimum = 128;
    decoder.maximum = 191;
    if decoder.remaining == 0 {
        return Step::Scalar(decoder.scalar);
    }
    return Step::Pending;
}

fn begin_scalar(decoder: &mut Decoder, value: i32, offset: i32) -> Step {
    if value <= 127 {
        return Step::Scalar(value);
    }
    if value < 194 || value > 244 {
        return fail(&mut *decoder, offset);
    }
    decoder.sequence_start = offset;
    decoder.minimum = 128;
    decoder.maximum = 191;
    if value <= 223 {
        decoder.scalar = value - 192;
        decoder.remaining = 1;
        return Step::Pending;
    }
    if value <= 239 {
        decoder.scalar = value - 224;
        decoder.remaining = 2;
        if value == 224 {
            decoder.minimum = 160;
        }
        if value == 237 {
            decoder.maximum = 159;
        }
        return Step::Pending;
    }
    decoder.scalar = value - 240;
    decoder.remaining = 3;
    if value == 240 {
        decoder.minimum = 144;
    }
    if value == 244 {
        decoder.maximum = 143;
    }
    return Step::Pending;
}

pub fn push_byte(decoder: &mut Decoder, byte: u8) -> Step {
    if decoder.state == 1 {
        return Step::Invalid(decoder.error_offset);
    }
    if decoder.state == 2 {
        return Step::PositionOverflow;
    }
    if decoder.state == 3 {
        return Step::Finished;
    }
    if decoder.offset == 2147483647 {
        decoder.state = 2;
        return Step::PositionOverflow;
    }
    let offset = decoder.offset;
    decoder.offset = offset + 1;
    let value = byte.to_i32();
    if decoder.remaining != 0 {
        return continue_scalar(&mut *decoder, value);
    }
    if value <= 127 {
        return Step::Scalar(value);
    }
    return begin_scalar(&mut *decoder, value, offset);
}

pub fn finish(decoder: &mut Decoder) -> Finish {
    if decoder.state == 1 {
        return Finish::Invalid(decoder.error_offset);
    }
    if decoder.state == 2 {
        return Finish::PositionOverflow;
    }
    if decoder.state == 3 {
        return Finish::Complete;
    }
    if decoder.remaining != 0 {
        decoder.state = 1;
        decoder.error_offset = decoder.sequence_start;
        return Finish::Invalid(decoder.error_offset);
    }
    decoder.state = 3;
    return Finish::Complete;
}
