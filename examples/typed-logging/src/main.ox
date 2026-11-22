use crate::logging::Level;
use crate::logging::Line;
use crate::logging::encode;
use std::io::WriteSomeStatus;
use std::io::write_some;

fn emit(bytes: &[u8]) -> i32 {
    let mut offset = 0;
    while offset < bytes.len() {
        let written = write_some(&*bytes[offset..bytes.len()]);
        match written {
            WriteSomeStatus::Written(count) => {
                if count <= 0 { return 1; }
                offset = offset + count;
            },
            WriteSomeStatus::Error(code) => { return 1; },
        }
    }
    return 0;
}

fn main() -> i32 {
    let message = b"service ready";
    let record = encode(Level::Info, &message);
    match record {
        Line::Encoded(bytes) => { return emit(&bytes); },
        Line::InvalidMessage(offset) => { return 2; },
        Line::ClockUnavailable => { return 3; },
        Line::CapacityOverflow => { return 4; },
        Line::AllocationFailed => { return 5; },
        Line::MemoryLimit => { return 6; },
    }
}
