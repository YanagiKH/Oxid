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
use crate::owned_vec::try_append;
use crate::owned_vec::pop;
use alloc::vec::Vec;

struct Message { bytes: ByteBuf }
enum Token { Empty, Data(ByteBuf), Wide(i128) }

fn push_message(messages: &mut Vec<Message>, value: u8) -> bool {
    let mut bytes = bytes_new();
    bytes_push(&mut bytes, value);
    let pushed = try_push(&mut *messages, Message { bytes: bytes });
    match pushed {
        PushResult::Pushed => { return true; },
        PushResult::Failed(failure) => { return false; },
    }
}

fn first_byte(bytes: &[u8]) -> i32 {
    if bytes.len() != 1 { return -2; }
    let value = bytes[0];
    return value.to_i32();
}

fn message_value(messages: &mut Vec<Message>) -> i32 {
    let selected = pop(&mut *messages);
    match selected {
        PopResult::Empty => { return -1; },
        PopResult::Value(message) => {
            let Message { bytes } = message;
            return first_byte(&bytes);
        },
    }
}

fn append_messages() -> bool {
    let mut destination = new::<Message>();
    let mut pending = new::<Message>();
    let mut buffer = new::<Message>();
    if !push_message(&mut destination, 10u8) || !push_message(&mut pending, 20u8)
        || !push_message(&mut pending, 30u8) || !push_message(&mut buffer, 40u8) { return false; }
    let appended = try_append(&mut destination, &mut pending, &mut buffer);
    match appended {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
    return len(&pending) == 0 && len(&buffer) == 1 && message_value(&mut buffer) == 40
        && message_value(&mut destination) == 30 && message_value(&mut destination) == 20
        && message_value(&mut destination) == 10 && len(&destination) == 0;
}

fn main() -> i32 {
    if !append_messages() { return -15; }
    let mut messages = new::<Message>();
    let initially_empty = pop(&mut messages);
    match initially_empty {
        PopResult::Empty => {},
        PopResult::Value(unexpected_message) => { return -1; },
    }
    if len(&messages) != 0 || capacity(&messages) != 0 { return -2; }
    let mut input_bytes = bytes_new();
    bytes_push(&mut input_bytes, 42u8);
    let pushed = try_push(&mut messages, Message { bytes: input_bytes });
    match pushed {
        PushResult::Pushed => {},
        PushResult::Failed(message_failure) => { return -3; },
    }
    let selected = pop(&mut messages);
    let mut answer = 0;
    match selected {
        PopResult::Empty => { return -4; },
        PopResult::Value(message) => {
            let Message { bytes } = message;
            answer = bytes_len(&bytes);
        },
    }
    let mut tokens = new::<Token>();
    let wide = try_push(&mut tokens, Token::Wide(170141183460469231731687303715884105727i128));
    match wide {
        PushResult::Pushed => {},
        PushResult::Failed(token_failure) => { return -5; },
    }
    let token = pop(&mut tokens);
    match token {
        PopResult::Empty => { return -6; },
        PopResult::Value(token_value) => {
            match token_value {
                Token::Wide(number) => {
                    if number != 170141183460469231731687303715884105727i128 { return -7; }
                    answer = answer + 40;
                },
                Token::Empty => { return -8; },
                Token::Data(data) => { return -9; },
            }
        },
    }
    let mut statuses = new::<GrowStatus>();
    let status_pushed = try_push(&mut statuses, GrowStatus::MemoryLimit);
    match status_pushed {
        PushResult::Pushed => {},
        PushResult::Failed(status_failure) => { return -10; },
    }
    let status = pop(&mut statuses);
    match status {
        PopResult::Empty => { return -11; },
        PopResult::Value(status_value) => {
            match status_value {
                GrowStatus::MemoryLimit => { answer = answer + 1; },
                GrowStatus::Ok => { return -12; },
                GrowStatus::AllocationFailed => { return -13; },
                GrowStatus::CapacityOverflow => { return -14; },
            }
        },
    }
    return answer;
}
