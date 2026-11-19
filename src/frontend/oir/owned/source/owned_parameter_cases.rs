//! The same ordinary package source feeds parsed, public and native tests.
pub const LIBRARY: &str = include_str!("../../../../../packages/owned_vec/src/lib.ox");
pub const EXAMPLE: &str = include_str!("../../../../../examples/owned-vector-results/src/main.ox");

pub const OWNERSHIP: &str = r#"
use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_push as copy_push;
use alloc::vec::GrowStatus;
use crate::owned_vec::transfer;
use crate::owned_vec::detach;
use crate::owned_vec::discard;
use crate::owned_vec::try_push;
use crate::owned_vec::pop;
use crate::owned_vec::PushResult;
use crate::owned_vec::PopResult;
struct Message { bytes: ByteBuf }
enum Token { Empty, Wide(i128) }
#[derive(Copy)] struct Point { value: i32 }
fn first_byte(bytes: &[u8]) -> u8 { return bytes[0]; }
fn main() -> i32 {
    let empty = new::<Message>();
    discard(transfer(empty));
    let mut messages = new::<Message>();
    let mut first = bytes_new();
    bytes_push(&mut first, 42u8);
    if bytes_len(&first) != 1 { return -1; }
    let first_push = try_push(&mut messages, Message { bytes: first });
    match first_push {
        PushResult::Pushed => {},
        PushResult::Failed(first_failure) => { return -2; },
    }
    let mut second = bytes_new();
    bytes_push(&mut second, 7u8);
    if bytes_len(&second) != 1 { return -3; }
    let second_push = try_push(&mut messages, Message { bytes: second });
    match second_push {
        PushResult::Pushed => {},
        PushResult::Failed(second_failure) => { return -4; },
    }
    let reserved = capacity(&messages);
    let mut moved = transfer(messages);
    let mut detached = detach(&mut moved);
    if len(&moved) != 0 || capacity(&moved) != 0 { return -5; }
    if len(&detached) != 2 || capacity(&detached) != reserved { return -6; }
    let second_pop = pop(&mut detached);
    match second_pop {
        PopResult::Empty => { return -7; },
        PopResult::Value(second_message) => {
            let Message { bytes: second_bytes } = second_message;
            if bytes_len(&second_bytes) != 1 || first_byte(&second_bytes) != 7u8 { return -8; }
        },
    }
    let first_pop = pop(&mut detached);
    match first_pop {
        PopResult::Empty => { return -9; },
        PopResult::Value(first_message) => {
            let Message { bytes: first_bytes } = first_message;
            if bytes_len(&first_bytes) != 1 || first_byte(&first_bytes) != 42u8 { return -10; }
            let reinserted = try_push(&mut detached, Message { bytes: first_bytes });
            match reinserted {
                PushResult::Pushed => {},
                PushResult::Failed(reinsert_failure) => { return -24; },
            }
        },
    }
    if len(&detached) != 1 || capacity(&detached) != reserved { return -25; }
    discard(detached);
    discard(moved);
    let mut tokens = new::<Token>();
    let token_push = try_push(&mut tokens, Token::Wide(170141183460469231731687303715884105727i128));
    match token_push {
        PushResult::Pushed => {},
        PushResult::Failed(token_failure) => { return -11; },
    }
    let mut token_owner = transfer(tokens);
    let mut token_batch = detach(&mut token_owner);
    let token_pop = pop(&mut token_batch);
    match token_pop {
        PopResult::Empty => { return -12; },
        PopResult::Value(token) => {
            match token {
                Token::Empty => { return -13; },
                Token::Wide(value) => {
                    if value != 170141183460469231731687303715884105727i128 { return -14; }
                },
            }
        },
    }
    let mut statuses = new::<GrowStatus>();
    let status_push = try_push(&mut statuses, GrowStatus::MemoryLimit);
    match status_push {
        PushResult::Pushed => {},
        PushResult::Failed(status_failure) => { return -15; },
    }
    discard(transfer(statuses));
    let mut numbers = new::<i32>();
    let number_push = copy_push(&mut numbers, 40);
    match number_push {
        GrowStatus::Ok => {},
        GrowStatus::MemoryLimit => { return -16; },
        GrowStatus::AllocationFailed => { return -17; },
        GrowStatus::CapacityOverflow => { return -18; },
    }
    let mut number_owner = transfer(numbers);
    let number_batch = detach(&mut number_owner);
    if len(&number_owner) != 0 || len(&number_batch) != 1 { return -19; }
    let mut points = new::<Point>();
    let point_push = copy_push(&mut points, Point { value: 2 });
    match point_push {
        GrowStatus::Ok => {},
        GrowStatus::MemoryLimit => { return -20; },
        GrowStatus::AllocationFailed => { return -21; },
        GrowStatus::CapacityOverflow => { return -22; },
    }
    let mut point_owner = transfer(points);
    let point_batch = detach(&mut point_owner);
    if len(&point_owner) != 0 || len(&point_batch) != 1 { return -23; }
    let point = point_batch[0];
    return number_batch[0] + point.value;
}
"#;

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub const FILE_OWNERSHIP: &str = r#"
use std::fs::File;
use std::fs::OpenResult;
use std::fs::create;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use crate::owned_vec::transfer;
use crate::owned_vec::detach;
use crate::owned_vec::discard;
use crate::owned_vec::try_push;
use crate::owned_vec::PushResult;
struct Handle { file: File }
fn main() -> i32 {
    let path = b__OWNED_VECTOR_FILE_PATH__;
    let opened = create(&path);
    match opened {
        OpenResult::Opened(file) => {
            let mut values = new::<Handle>();
            let pushed = try_push(&mut values, Handle { file: file });
            match pushed {
                PushResult::Pushed => {},
                PushResult::Failed(failure) => { return -1; },
            }
            let mut moved = transfer(values);
            let detached = detach(&mut moved);
            if len(&moved) != 0 || capacity(&moved) != 0 { return -2; }
            if len(&detached) != 1 { return -3; }
            discard(detached);
            return 42;
        },
        OpenResult::Error(code) => { return -4; },
        OpenResult::MemoryLimit => { return -5; },
        OpenResult::AllocationFailed => { return -6; },
    }
}
"#;

pub const FAILURE: &str = r#"
use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::GrowStatus;
use crate::owned_vec::PushFailure;
use crate::owned_vec::PushResult;
use crate::owned_vec::PopResult;
use crate::owned_vec::try_push;
use crate::owned_vec::pop;
enum Parcel { Empty, Data(ByteBuf) }
fn first_byte(bytes: &[u8]) -> u8 { return bytes[0]; }
fn main() -> i32 {
    let mut first = bytes_new();
    bytes_push(&mut first, 41u8);
    let mut values = new::<Parcel>();
    let pushed = try_push(&mut values, Parcel::Data(first));
    match pushed {
        PushResult::Pushed => {},
        PushResult::Failed(first_failure) => { return -1; },
    }
    let reserved = capacity(&values);
    while len(&values) < reserved {
        let filled = try_push(&mut values, Parcel::Empty);
        match filled {
            PushResult::Pushed => {},
            PushResult::Failed(fill_failure) => { return -2; },
        }
    }
    let mut next = bytes_new();
    bytes_push(&mut next, 42u8);
    let result = try_push(&mut values, Parcel::Data(next));
    match result {
        PushResult::Pushed => { return 0; },
        PushResult::Failed(failure) => {
            let PushFailure { value, status } = failure;
            let mut answer = 0;
            match status {
                GrowStatus::AllocationFailed => { answer = 40; },
                GrowStatus::MemoryLimit => { answer = 41; },
                GrowStatus::CapacityOverflow => { return -3; },
                GrowStatus::Ok => { return -4; },
            }
            if len(&values) != reserved || capacity(&values) != reserved { return -5; }
            match value {
                Parcel::Empty => { return -6; },
                Parcel::Data(incoming_bytes) => {
                    if bytes_len(&incoming_bytes) != 1 || first_byte(&incoming_bytes) != 42u8 { return -7; }
                    answer = answer + bytes_len(&incoming_bytes);
                },
            }
            while len(&values) > 1 {
                let selected = pop(&mut values);
                match selected {
                    PopResult::Empty => { return -8; },
                    PopResult::Value(extra_item) => {
                        match extra_item {
                            Parcel::Empty => {},
                            Parcel::Data(extra_bytes) => { return -9; },
                        }
                    },
                }
            }
            let last = pop(&mut values);
            match last {
                PopResult::Empty => { return -10; },
                PopResult::Value(original_item) => {
                    match original_item {
                        Parcel::Empty => { return -11; },
                        Parcel::Data(original_bytes) => {
                            if bytes_len(&original_bytes) != 1 || first_byte(&original_bytes) != 41u8 { return -12; }
                            answer = answer + bytes_len(&original_bytes);
                        },
                    }
                },
            }
            let empty = pop(&mut values);
            match empty {
                PopResult::Empty => {},
                PopResult::Value(unexpected_item) => { return -13; },
            }
            if len(&values) != 0 || capacity(&values) != reserved { return -14; }
            return answer;
        },
    }
}
"#;
