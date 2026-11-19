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
use crate::text::AppendStatus;
use crate::text::new_text;
use crate::text::from_utf8;
use crate::text::append_utf8;
use crate::text::byte_len;
use crate::text::byte_capacity;
use crate::text::scalar_count;
use crate::text::equals_utf8;

enum Value { Empty, Number(i32), Text(Text) }
struct Entry { value: Value }

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn appended(status: AppendStatus) -> bool {
    match status {
        AppendStatus::Ok => { return true; },
        AppendStatus::InvalidInput(offset) => { return false; },
        AppendStatus::CapacityOverflow => { return false; },
        AppendStatus::AllocationFailed => { return false; },
        AppendStatus::MemoryLimit => { return false; },
    }
}

fn check_text_vector() -> i32 {
    let first_bytes = b"hello";
    let second_bytes = b"\0A¢€🦀é";
    let mut first = new_text();
    let mut second = new_text();
    if !appended(append_utf8(&mut first, &first_bytes)) { return 1; }
    if !appended(append_utf8(&mut second, &second_bytes)) { return 2; }
    let mut texts = new::<Text>();
    if !grew(try_reserve(&mut texts, 2)) { return 3; }
    if capacity(&texts) < 2 { return 4; }
    push_reserved(&mut texts, first);
    push_reserved(&mut texts, second);
    let last = pop_last(&mut texts);
    if byte_len(&last) != 14 || scalar_count(&last) != 7 { return 5; }
    if !equals_utf8(&last, &second_bytes) { return 6; }
    push_reserved(&mut texts, last);
    let mut moved = take(&mut texts);
    if len(&texts) != 0 || capacity(&texts) != 0 || len(&moved) != 2 { return 7; }
    truncate(&mut moved, 1);
    if len(&moved) != 1 { return 8; }
    let restored = pop_last(&mut moved);
    if !equals_utf8(&restored, &first_bytes) { return 9; }
    if byte_len(&restored) != 5 || byte_capacity(&restored) < 5 { return 10; }
    truncate(&mut moved, 0);
    return 0;
}

fn empty_entry(entry: Entry) -> bool {
    let Entry { value: value } = entry;
    match value {
        Value::Empty => { return true; },
        Value::Number(number) => { return false; },
        Value::Text(text) => { return false; },
    }
}

fn number_entry(entry: Entry) -> bool {
    let Entry { value: value } = entry;
    match value {
        Value::Empty => { return false; },
        Value::Number(number) => { return number == 17; },
        Value::Text(text) => { return false; },
    }
}

fn text_entry(entry: Entry) -> bool {
    let Entry { value: value } = entry;
    match value {
        Value::Empty => { return false; },
        Value::Number(number) => { return false; },
        Value::Text(text) => {
            let expected = b"🦀";
            return byte_len(&text) == 4 && scalar_count(&text) == 1 && equals_utf8(&text, &expected);
        },
    }
}

fn collect_entries(text: Text) -> i32 {
    let mut entries = new::<Entry>();
    if !grew(try_reserve(&mut entries, 3)) { return 11; }
    push_reserved(&mut entries, Entry { value: Value::Text(text) });
    push_reserved(&mut entries, Entry { value: Value::Number(17) });
    push_reserved(&mut entries, Entry { value: Value::Empty });
    let mut moved = take(&mut entries);
    if len(&entries) != 0 || capacity(&entries) != 0 || len(&moved) != 3 { return 12; }
    if !empty_entry(pop_last(&mut moved)) { return 13; }
    if !number_entry(pop_last(&mut moved)) { return 14; }
    if !text_entry(pop_last(&mut moved)) { return 15; }
    truncate(&mut moved, 0);
    return 0;
}

fn check_text_enum() -> i32 {
    let bytes = b"🦀";
    let converted = from_utf8(&bytes);
    match converted {
        FromUtf8::Ok(text) => { return collect_entries(text); },
        FromUtf8::InvalidInput(offset) => { return 16; },
        FromUtf8::CapacityOverflow => { return 17; },
        FromUtf8::AllocationFailed => { return 18; },
        FromUtf8::MemoryLimit => { return 19; },
    }
}

fn main() -> i32 {
    let direct = check_text_vector();
    if direct != 0 { return direct; }
    return check_text_enum();
}
