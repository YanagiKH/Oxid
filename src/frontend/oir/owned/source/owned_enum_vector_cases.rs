//! Shared literal source fixtures for public and sealed-consumer regressions.
pub const API: &str = r#"
use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::Vec;
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;
fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}
"#;

pub const INTERLEAVED_BUILTINS: &str = r#"
use std::io::ReadStatus;
enum User<T> { None, Some(T) }
fn wrap<T>(value: T) -> User<T> { return User::Some(value); }
fn main() -> i32 {
    let mut statuses = new::<GrowStatus>();
    if !grew(try_reserve(&mut statuses, 2)) { return -1; }
    push_reserved(&mut statuses, GrowStatus::Ok);
    let mut reads = new::<ReadStatus>();
    if !grew(try_reserve(&mut reads, 1)) { return -2; }
    push_reserved(&mut reads, ReadStatus::Eof(2));
    let mut small = new::<User<i32>>();
    if !grew(try_reserve(&mut small, 1)) { return -3; }
    push_reserved(&mut small, wrap(40));
    let wide_input = wrap(170141183460469231731687303715884105727i128);
    let mut wide = new::<User<i128>>();
    if !grew(try_reserve(&mut wide, 1)) { return -4; }
    push_reserved(&mut wide, wide_input);
    push_reserved(&mut statuses, GrowStatus::MemoryLimit);
    let last_status = pop_last(&mut statuses);
    match last_status {
        GrowStatus::MemoryLimit => {},
        GrowStatus::Ok => { return -5; },
        GrowStatus::CapacityOverflow => { return -6; },
        GrowStatus::AllocationFailed => { return -7; },
    }
    let first_status = pop_last(&mut statuses);
    match first_status {
        GrowStatus::Ok => {},
        GrowStatus::MemoryLimit => { return -8; },
        GrowStatus::CapacityOverflow => { return -9; },
        GrowStatus::AllocationFailed => { return -10; },
    }
    let mut transferred = take(&mut reads);
    let mut count = 0;
    let last_read = pop_last(&mut transferred);
    match last_read {
        ReadStatus::Eof(number) => { count = number; },
        ReadStatus::Full => { return -11; },
        ReadStatus::IoError => { return -12; },
    }
    let last_wide = pop_last(&mut wide);
    match last_wide {
        User::None => { return -13; },
        User::Some(number) => {
            if number != 170141183460469231731687303715884105727i128 { return -14; }
        },
    }
    truncate(&mut statuses, 0);
    if len(&reads) != 0 || len(&statuses) != 0 || len(&wide) != 0 { return -15; }
    let last_small = pop_last(&mut small);
    match last_small {
        User::None => { return -16; },
        User::Some(number) => { return number + count; },
    }
}
"#;

pub const RECORD_FIELD_LIFETIME: &str = r#"
enum Entry { Empty, Data(ByteBuf), Wide(i128) }
struct Holder { values: Vec<Entry>, other: ByteBuf }
fn make() -> Holder {
    let mut payload = bytes_new();
    bytes_push(&mut payload, 42u8);
    let mut values = new::<Entry>();
    if !grew(try_reserve(&mut values, 1)) {
        return Holder { values: values, other: payload };
    }
    push_reserved(&mut values, Entry::Data(payload));
    let mut other = bytes_new();
    bytes_push(&mut other, 1u8);
    return Holder { values: values, other: other };
}
fn main() -> i32 {
    let mut first = make();
    let second = make();
    first = second;
    let Holder { values, other } = first;
    let mut moved = values;
    if len(&moved) != 1 || bytes_len(&other) != 1 { return -1; }
    let last_entry = pop_last(&mut moved);
    match last_entry {
        Entry::Data(payload) => {
            if bytes_len(&payload) != 1 { return -2; }
            return 42;
        },
        Entry::Empty => { return -3; },
        Entry::Wide(number) => { return -4; },
    }
}
"#;

pub const MEMORY_LIMIT: &str = r#"
enum Parcel { Empty, Data(ByteBuf) }
fn main() -> i32 {
    let mut first = bytes_new();
    if !grew(bytes_push(&mut first, 42u8)) { return -1; }
    let mut next = bytes_new();
    if !grew(bytes_push(&mut next, 43u8)) { return -2; }
    let incoming = Parcel::Data(next);
    let mut values = new::<Parcel>();
    if !grew(try_reserve(&mut values, 1)) { return -3; }
    push_reserved(&mut values, Parcel::Data(first));
    let original_capacity = capacity(&values);
    let growth = try_reserve(&mut values, 127);
    match growth {
        GrowStatus::MemoryLimit => {},
        GrowStatus::Ok => { return -4; },
        GrowStatus::CapacityOverflow => { return -5; },
        GrowStatus::AllocationFailed => { return -6; },
    }
    if len(&values) != 1 || capacity(&values) != original_capacity { return -7; }
    let mut total = 40;
    let last_parcel = pop_last(&mut values);
    match last_parcel {
        Parcel::Data(payload) => { total = total + bytes_len(&payload); },
        Parcel::Empty => { return -8; },
    }
    match incoming {
        Parcel::Data(payload) => { total = total + bytes_len(&payload); },
        Parcel::Empty => { return -9; },
    }
    return total;
}
"#;

pub const JOINT_RECORD_ENUM_FIELDS: &str = r#"
enum Entry { Value(i32) }
enum Marker { Ready }
struct Holder { values: Vec<Entry>, marker: Marker }
fn main() -> i32 {
    let mut pending = new::<Entry>();
    if !grew(try_reserve(&mut pending, 1)) { return -1; }
    push_reserved(&mut pending, Entry::Value(42));
    let holder = Holder { values: pending, marker: Marker::Ready };
    let Holder { values, marker } = holder;
    let mut detached = values;
    match marker {
        Marker::Ready => {},
    }
    let selected = pop_last(&mut detached);
    match selected {
        Entry::Value(number) => { return number; },
    }
}
"#;

pub const SPARSE_ARGUMENT_STATUS: &str = r#"
use std::env::ArgStatus;
enum Later<T> { Item(T) }
fn later<T>(value: T) -> Later<T> { return Later::Item(value); }
fn main() -> i32 {
    let mut statuses = new::<ArgStatus>();
    if !grew(try_reserve(&mut statuses, 1)) { return -1; }
    push_reserved(&mut statuses, ArgStatus::Copied(41));
    let small = later(1);
    let wide = later(170141183460469231731687303715884105727i128);
    let mut small_values = new::<Later<i32>>();
    if !grew(try_reserve(&mut small_values, 1)) { return -2; }
    push_reserved(&mut small_values, small);
    let mut wide_values = new::<Later<i128>>();
    if !grew(try_reserve(&mut wide_values, 1)) { return -3; }
    push_reserved(&mut wide_values, wide);
    let last_wide = pop_last(&mut wide_values);
    match last_wide {
        Later::Item(number) => {
            if number != 170141183460469231731687303715884105727i128 { return -4; }
        },
    }
    let mut result = 0;
    let last_small = pop_last(&mut small_values);
    match last_small {
        Later::Item(number) => { result = number; },
    }
    let last_argument = pop_last(&mut statuses);
    match last_argument {
        ArgStatus::Copied(number) => { return result + number; },
        ArgStatus::TooSmall(required) => { return -5; },
        ArgStatus::Missing => { return -6; },
        ArgStatus::TooLong => { return -7; },
    }
}
"#;
