use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_push;
use alloc::bytes::try_extend;
use std::io::write_some;
use std::io::WriteSomeStatus;

fn grown(status: GrowStatus) -> bool {
    match status { GrowStatus::Ok => { return true; }, GrowStatus::CapacityOverflow => { return false; }, GrowStatus::AllocationFailed => { return false; }, GrowStatus::MemoryLimit => { return false; } }
}
fn append(bytes: &mut ByteBuf, text: &[u8]) -> bool { return grown(try_extend(&mut *bytes, &*text)); }
fn push(bytes: &mut ByteBuf, byte: u8) -> bool { return grown(try_push(&mut *bytes, byte)); }

pub fn write(text: &[u8]) -> bool {
    let mut offset = 0;
    while offset < text.len() {
        let written = write_some(&*text[offset..text.len()]);
        match written {
            WriteSomeStatus::Written(count) => { if count <= 0 { return false; } offset = offset + count; },
            WriteSomeStatus::Error(code) => { if code != 4 { return false; } }
        }
    }
    return true;
}

fn unsigned(bytes: &mut ByteBuf, number: u64) -> bool {
    let mut digits = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let mut remaining = number;
    let mut used = 0;
    while true {
        let digit = remaining % 10u64;
        digits[used] = digit.to_u8_checked() + 48u8;
        used = used + 1;
        remaining = remaining / 10u64;
        if remaining == 0u64 { break; }
    }
    for index in 0..used { if !push(&mut *bytes, digits[used - index - 1]) { return false; } }
    return true;
}

fn scientific(bytes: &mut ByteBuf, number: f64) -> bool {
    if number != number { let text = b"NaN"; return append(&mut *bytes, &text); }
    let bits = number.to_bits();
    if (bits >> 63) != 0u64 { if !push(&mut *bytes, 45u8) { return false; } }
    let mut magnitude = number;
    if (bits >> 63) != 0u64 { magnitude = -magnitude; }
    if ((bits >> 52) & 2047u64) == 2047u64 { let text = b"inf"; return append(&mut *bytes, &text); }
    if magnitude == 0.0 { let text = b"0.00000e0"; return append(&mut *bytes, &text); }
    let mut exponent = 0;
    while magnitude >= 10.0 { magnitude = magnitude / 10.0; exponent = exponent + 1; }
    while magnitude < 1.0 { magnitude = magnitude * 10.0; exponent = exponent - 1; }
    magnitude = magnitude + 0.000005;
    if magnitude >= 10.0 { magnitude = magnitude / 10.0; exponent = exponent + 1; }
    for place in 0..6 {
        let mut digit = 0u8;
        while magnitude >= 1.0 { magnitude = magnitude - 1.0; digit = digit + 1u8; }
        if !push(&mut *bytes, digit + 48u8) { return false; }
        if place == 0 { if !push(&mut *bytes, 46u8) { return false; } }
        magnitude = magnitude * 10.0;
    }
    if !push(&mut *bytes, 101u8) { return false; }
    if exponent < 0 { if !push(&mut *bytes, 45u8) { return false; } exponent = -exponent; }
    return unsigned(&mut *bytes, exponent.to_u64_checked());
}

pub fn summary(updates: u64, loss: f64, path: &[u8]) -> i32 {
    let mut bytes = new();
    let updates_label = b"Batches: "; let loss_label = b"\nMSE: "; let checkpoint_label = b"\nCheckpoint: ";
    if !append(&mut bytes, &updates_label) || !unsigned(&mut bytes, updates) || !append(&mut bytes, &loss_label) || !scientific(&mut bytes, loss) || !append(&mut bytes, &checkpoint_label) || !append(&mut bytes, &*path) || !push(&mut bytes, 10u8) { return 1; }
    if !write(&bytes) { return 1; }
    return 0;
}
fn intro() -> bool {
    let text = b"Oxid minibatch training\n\nTrain paired data with shuffled batches and resume from the saved data position.\n\n";
    return write(&text);
}
fn usage() -> bool {
    let text = b"  minibatch-training train <dataset> <checkpoint-prefix> <steps> <batch-size> [sgd|adam|adamw] [seed]\n  minibatch-training resume <checkpoint-prefix> <steps>\n\n";
    return write(&text);
}
fn details() -> bool {
    let text = b"Steps: 1 to 1000000. Batch size: 1 to 2147483647. Shuffle seed: 0 to 4294967295; default 42. Default optimizer: SGD (rate 0.05).\nAdam and AdamW use rate 0.01. Resume restores the model, optimizer, data and next batch.\n";
    return write(&text);
}
fn checkpoints() -> bool {
    let text = b"Two checkpoint files are saved every 25 updates and at completion.\n";
    return write(&text);
}
pub fn help() -> i32 {
    if !intro() || !usage() || !details() || !checkpoints() { return 1; }
    return 0;
}

pub fn failure() -> i32 {
    let text = b"Training failed. Check the checkpoint paths, available memory and file permissions.\n"; write(&text); return 1;
}
pub fn existing() -> i32 {
    let text = b"A checkpoint already exists. Use resume or choose a new prefix.\n"; write(&text); return 2;
}
