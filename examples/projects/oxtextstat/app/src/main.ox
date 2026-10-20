use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use stream::Decoder;
use stream::new_decoder;
use stat::Counts;
use stat::new_counts;
use stat::feed;
use stat::finalize;
use stat::render;

// Statuses: 0 report, 65 malformed/incomplete UTF-8, 70 checked overflow or
// unexpected lifecycle result, 71 buffer allocation/memory refusal, 74 I/O.
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

// One retained decoder across every read; each returned byte is fed once.
// finalize runs only on real EOF.
fn consume(buffer: &mut [u8], decoder: &mut Decoder, counts: &mut Counts) -> i32 {
    let capacity = buffer.len();
    while true {
        let read = read_some(&mut *buffer);
        match read {
            ReadSomeStatus::Data(count) => {
                if count <= 0 || count > capacity { return 70; }
                let status = feed(&mut *decoder, &mut *counts, &*buffer[0..count]);
                if status != 0 { return status; }
            },
            ReadSomeStatus::Eof => {
                return finalize(&mut *decoder, &mut *counts);
            },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return 74; }
            },
        }
    }
    return 74;
}

// One 64-byte read buffer, initialized once; after EOF it holds the report
// (at most 33 bytes). The small size keeps multi-read inputs within fuel.
fn main() -> i32 {
    let zero = 0;
    let mut buffer = new();
    let ready = try_resize(&mut buffer, 64, zero.to_u8_checked());
    let ready_status = grow_status(ready);
    if ready_status != 0 { return ready_status; }
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    let consumed = consume(&mut buffer, &mut decoder, &mut counts);
    if consumed != 0 { return consumed; }
    let length = render(&counts, &mut buffer);
    if length <= 0 { return 70; }
    return write_all(&buffer[0..length]);
}
