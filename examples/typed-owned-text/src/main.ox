use alloc::bytes::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;
use crate::text::Text;
use crate::text::FromUtf8;
use crate::text::from_utf8;
use crate::text::append_text;
use crate::text::byte_len;
use crate::text::byte_capacity;
use crate::text::scalar_count;
use crate::text::equals_utf8;
use crate::text::try_reserve_bytes;

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn store_greeting(greeting: Text) -> i32 {
    let mut saved = new::<Text>();
    if !grew(try_reserve(&mut saved, 2)) { return -5; }
    if capacity(&saved) < 2 { return -6; }
    push_reserved(&mut saved, greeting);
    if len(&saved) != 1 { return -7; }
    let first = pop_last(&mut saved);
    if byte_len(&first) != 16 || scalar_count(&first) != 13 { return -8; }
    push_reserved(&mut saved, first);
    let mut moved = take(&mut saved);
    if len(&saved) != 0 || capacity(&saved) != 0 { return -9; }
    let restored = pop_last(&mut moved);
    let length = byte_len(&restored);
    push_reserved(&mut moved, restored);
    truncate(&mut moved, 0);
    if len(&moved) != 0 { return -10; }
    return length;
}

fn finish_greeting(prefix: Text) -> i32 {
    let mut greeting = prefix;
    let suffix_bytes = b"Oxid 🦀";
    let suffix_result = from_utf8(&suffix_bytes);
    match suffix_result {
        FromUtf8::Ok(suffix) => {
            if !grew(try_reserve_bytes(&mut greeting, byte_len(&suffix))) { return -11; }
            if byte_capacity(&greeting) < 16 { return -12; }
            if !grew(append_text(&mut greeting, &suffix)) { return -13; }
            if !equals_utf8(&suffix, &suffix_bytes) { return -14; }
            return store_greeting(greeting);
        },
        FromUtf8::InvalidInput(offset) => { return -1; },
        FromUtf8::CapacityOverflow => { return -2; },
        FromUtf8::AllocationFailed => { return -3; },
        FromUtf8::MemoryLimit => { return -4; },
    }
}

fn main() -> i32 {
    let prefix_bytes = b"Hello, ";
    let prefix_result = from_utf8(&prefix_bytes);
    match prefix_result {
        FromUtf8::Ok(prefix) => { return finish_greeting(prefix); },
        FromUtf8::InvalidInput(offset) => { return -1; },
        FromUtf8::CapacityOverflow => { return -2; },
        FromUtf8::AllocationFailed => { return -3; },
        FromUtf8::MemoryLimit => { return -4; },
    }
}
