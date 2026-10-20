use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::try_extend;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
use std::io::read_some;
use std::io::write_some;
use sum::new_summary;
use sum::summarize;
use sum::render;

// Statuses: 0 report, 65 invalid UTF-8/CSV/field count/decimal, 70 checked
// overflow, capacity overflow or impossible result, 71 allocation or
// memory-policy refusal, 74 read/write error or zero-progress write.
enum Loaded {
    Ok(ByteBuf),
    Failed(i32)
}

fn grow_status(status: GrowStatus) -> i32 {
    match status {
        GrowStatus::Ok => { return 0; },
        GrowStatus::CapacityOverflow => { return 70; },
        GrowStatus::AllocationFailed => { return 71; },
        GrowStatus::MemoryLimit => { return 71; },
    }
}

// Reads stdin to EOF into one growable owner through read_some, retrying only
// EINTR (4). This is a complete-buffer program, not a streaming summary.
fn read_all() -> Loaded {
    let mut bytes = new();
    let mut chunk = new();
    let zero = 0;
    let ready = grow_status(try_resize(&mut chunk, 64, zero.to_u8_checked()));
    if ready != 0 { return Loaded::Failed(ready); }
    while true {
        let read = read_some(&mut chunk);
        match read {
            ReadSomeStatus::Data(count) => {
                if count <= 0 || count > 64 { return Loaded::Failed(70); }
                let appended = grow_status(try_extend(&mut bytes, &chunk[0..count]));
                if appended != 0 { return Loaded::Failed(appended); }
            },
            ReadSomeStatus::Eof => { return Loaded::Ok(bytes); },
            ReadSomeStatus::Error(code) => {
                if code != 4 { return Loaded::Failed(74); }
            },
        }
    }
    return Loaded::Failed(74);
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

// Nothing is written until the whole input has been read and validated and
// the report has been rendered.
fn main() -> i32 {
    let loaded = read_all();
    let mut summary = new_summary();
    match loaded {
        Loaded::Ok(input) => {
            let status = summarize(input, &mut summary);
            if status != 0 { return status; }
        },
        Loaded::Failed(status) => { return status; },
    }
    let mut report = b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
    let length = render(&summary, &mut report);
    if length <= 0 { return 70; }
    return write_all(&report[0..length]);
}
