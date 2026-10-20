use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use hex::DecodeResult;
use hex::decode;

// Statuses: 0 success, 65 malformed input or odd EOF, 70 impossible contract,
// 71 allocation failure, 74 I/O failure.
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

// Decode one complete even-length view. An invalid view emits none of its bytes.
fn decode_chunk(input: &[u8], output: &mut [u8]) -> i32 {
    let decoded = decode(&*input, &mut *output);
    match decoded {
        DecodeResult::Written(count) => {
            if count != input.len() / 2 || count > output.len() { return 70; }
            return write_all(&*output[0..count]);
        },
        DecodeResult::OddLength => { return 70; },
        DecodeResult::InvalidDigit(offset) => { return 65; },
        DecodeResult::BufferTooSmall(required) => { return 70; },
    }
}

// staging[0] may hold one carried byte; each read adds at most 1,024 new bytes.
fn decode_stream(staging: &mut [u8], output: &mut [u8]) -> i32 {
    let mut carry = 0;
    while true {
        let limit = carry + 1024;
        if limit > staging.len() { return 70; }
        let read = read_some(&mut *staging[carry..limit]);
        match read {
            ReadSomeStatus::Data(count) => {
                if count <= 0 || count > 1024 { return 70; }
                let total = carry + count;
                let even = total - total % 2;
                let status = decode_chunk(&*staging[0..even], &mut *output);
                if status != 0 { return status; }
                carry = total - even;
                if carry == 1 {
                    let last = staging[even];
                    staging[0] = last;
                }
            },
            ReadSomeStatus::Eof => {
                if carry != 0 { return 65; }
                return 0;
            },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return 74; }
            },
        }
    }
    return 74;
}

// Fixed storage: 1,025 staging bytes and 512 output bytes, initialized once.
fn main() -> i32 {
    let zero = 0;
    let mut staging = new();
    let staging_ready = try_resize(&mut staging, 1025, zero.to_u8_checked());
    let staging_status = grow_status(staging_ready);
    if staging_status != 0 { return staging_status; }
    let mut output = new();
    let output_ready = try_resize(&mut output, 512, zero.to_u8_checked());
    let output_status = grow_status(output_ready);
    if output_status != 0 { return output_status; }
    return decode_stream(&mut staging, &mut output);
}
