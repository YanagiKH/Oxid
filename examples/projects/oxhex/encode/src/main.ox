use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use hex::EncodeResult;
use hex::encode_lower;

// Statuses: 0 success, 70 impossible contract, 71 allocation failure, 74 I/O failure.
fn grow_status(status: GrowStatus) -> i32 {
    match status {
        GrowStatus::Ok => { return 0; },
        GrowStatus::CapacityOverflow => { return 70; },
        GrowStatus::AllocationFailed => { return 71; },
        GrowStatus::MemoryLimit => { return 71; },
    }
}

// Drain exactly `bytes`, retrying only EINTR (4); zero progress is an I/O failure.
fn write_all(bytes: &[u8]) -> i32 {
    let mut offset = 0;
    let length = bytes.len();
    while offset < length {
        let written = write_some(&*bytes[offset..length]);
        match written {
            WriteSomeStatus::Written(count) => {
                if count < 0 || count > length - offset { return 70; }
                if count == 0 { return 74; }
                offset = offset + count;
            },
            WriteSomeStatus::Error(code) => {
                if code != 4 { return 74; }
            },
        }
    }
    return 0;
}

// Encode one read prefix and drain exactly the `Written` prefix of `output`.
fn encode_chunk(input: &[u8], output: &mut [u8]) -> i32 {
    let encoded = encode_lower(&*input, &mut *output);
    match encoded {
        EncodeResult::Written(count) => {
            if count != input.len() * 2 || count > output.len() { return 70; }
            return write_all(&*output[0..count]);
        },
        EncodeResult::BufferTooSmall(required) => { return 70; },
        EncodeResult::LengthOverflow => { return 70; },
    }
}

fn encode_stream(input: &mut [u8], output: &mut [u8]) -> i32 {
    while true {
        let read = read_some(&mut *input);
        match read {
            ReadSomeStatus::Data(count) => {
                if count <= 0 || count > input.len() { return 70; }
                let status = encode_chunk(&*input[0..count], &mut *output);
                if status != 0 { return status; }
            },
            ReadSomeStatus::Eof => { return 0; },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return 74; }
            },
        }
    }
    return 74;
}

// Fixed storage: 1,024 input bytes and 2,048 output bytes, initialized once.
fn main() -> i32 {
    let zero = 0;
    let mut input = new();
    let input_ready = try_resize(&mut input, 1024, zero.to_u8_checked());
    let input_status = grow_status(input_ready);
    if input_status != 0 { return input_status; }
    let mut output = new();
    let output_ready = try_resize(&mut output, 2048, zero.to_u8_checked());
    let output_status = grow_status(output_ready);
    if output_status != 0 { return output_status; }
    return encode_stream(&mut input, &mut output);
}
