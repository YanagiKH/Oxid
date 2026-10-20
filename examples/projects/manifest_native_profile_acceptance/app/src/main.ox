use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use model::base;
use model::scale;
use model::shapes::make_pair;
use model::shapes::pair_sum;
use model::shapes::measure;
use model::shapes::score;
use model::shapes::filled;
use model::shapes::byte_total;
use text::FromBytes;
use text::from_bytes;
use text::byte_len;
use text::scalar_count;

// Every expected value is written by hand (see README.md). With base() == 7:
// scalar 21 + record 9 + enum 2 + owned buffer 6 + Text 4 == 42.
fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}
fn scalar_part() -> i32 {
    return scale(base());
}
fn record_part() -> i32 {
    let pair = make_pair(base(), 2);
    return pair_sum(&pair);
}
fn enum_part() -> i32 {
    return score(measure(base(), 1)) + score(measure(-6, 9));
}
fn buffer_part() -> i32 {
    let owner = filled();
    return byte_total(&owner);
}
// "Ox" followed by U+00A2: 4 bytes, 3 scalars.
fn text_bytes() -> i32 {
    let mut owner = new();
    let bytes = [byte(79), byte(120), byte(194), byte(162)];
    let grown = try_extend(&mut owner, &bytes);
    match grown {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => {
            return -100;
        },
        GrowStatus::AllocationFailed => {
            return -101;
        },
        GrowStatus::MemoryLimit => {
            return -102;
        },
    }
    let parsed = from_bytes(owner);
    match parsed {
        FromBytes::Ok(text) => {
            return byte_len(&text) * 10 + scalar_count(&text);
        },
        FromBytes::Invalid(rest) => {
            return -1 - len(&rest);
        },
    }
}
fn text_part() -> i32 {
    return text_bytes() / 10;
}
fn main() -> i32 {
    return scalar_part() + record_part() + enum_part() + buffer_part() + text_part();
}
fn test_scalar_scale() -> bool {
    return scalar_part() == 21;
}
fn test_record_pair() -> bool {
    return record_part() == 9;
}
fn test_enum_outcomes() -> bool {
    return enum_part() == 2;
}
fn test_buffer_bytes() -> i32 {
    return buffer_part() - 6;
}
fn test_text_scalars() -> bool {
    return text_bytes() == 43;
}
fn test_unit_tail() -> () {
    return;
}
