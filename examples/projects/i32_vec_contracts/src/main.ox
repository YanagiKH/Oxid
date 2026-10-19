use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::len as bytes_len;
use alloc::bytes::capacity;
use alloc::bytes::try_reserve;
use alloc::bytes::try_extend;
use vec::I32Vec;
use vec::Element;
use vec::FromBytes;
use vec::new_vec;
use vec::len;
use vec::get;
use vec::set;
use vec::try_push;
use vec::pop;
use vec::from_le_bytes;
use vec::into_le_bytes;

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn equals_value(element: Element, expected: i32) -> bool {
    match element {
        Element::Value(actual) => { return actual == expected; },
        Element::Missing => { return false; },
    }
}

fn missing(element: Element) -> bool {
    match element {
        Element::Value(actual) => { return false; },
        Element::Missing => { return true; },
    }
}

fn same_bytes(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() { return false; }
    let mut offset = 0;
    while offset < actual.len() {
        if actual[offset] != expected[offset] { return false; }
        offset = offset + 1;
    }
    return true;
}

fn return_vector(input: I32Vec) -> I32Vec {
    return input;
}

fn return_owner(input: ByteBuf) -> ByteBuf {
    return input;
}

fn test_empty_and_bounds() -> i32 {
    let mut vector = new_vec();
    if len(&vector) != 0 { return 101; }
    if !missing(get(&vector, -1)) { return 102; }
    if !missing(get(&vector, 0)) { return 103; }
    if !missing(get(&vector, 2147483647)) { return 104; }
    if !missing(pop(&mut vector)) { return 105; }
    if set(&mut vector, -1, 7) { return 106; }
    if set(&mut vector, 0, 7) { return 107; }
    if !grew(try_push(&mut vector, 17)) { return 108; }
    if set(&mut vector, 2147483647, 7) { return 109; }
    if set(&mut vector, -2147483647 - 1, 7) { return 110; }
    if !missing(get(&vector, -2147483647 - 1)) { return 111; }
    if !missing(get(&vector, 1)) { return 112; }
    if !equals_value(get(&vector, 0), 17) { return 113; }
    if len(&vector) != 1 { return 114; }
    if !equals_value(pop(&mut vector), 17) { return 115; }
    if !missing(pop(&mut vector)) { return 116; }
    return 0;
}

fn test_set_preserves_neighbors() -> i32 {
    let mut vector = new_vec();
    if !grew(try_push(&mut vector, 17)) { return 201; }
    if !grew(try_push(&mut vector, 19)) { return 202; }
    if !grew(try_push(&mut vector, 23)) { return 203; }
    if !set(&mut vector, 1, -2147483647 - 1) { return 204; }
    if !equals_value(get(&vector, 0), 17) { return 205; }
    if !equals_value(get(&vector, 1), -2147483647 - 1) { return 206; }
    if !equals_value(get(&vector, 2), 23) { return 207; }
    if set(&mut vector, 3, 7) { return 208; }
    if set(&mut vector, -1, 7) { return 209; }
    if len(&vector) != 3 { return 210; }
    let expected = [byte(17), byte(0), byte(0), byte(0), byte(0), byte(0), byte(0), byte(128), byte(23), byte(0), byte(0), byte(0)];
    let owner = into_le_bytes(vector);
    if !same_bytes(&owner, &expected) { return 211; }
    return 0;
}

fn reserved_empty_result(input: ByteBuf, expected_capacity: i32) -> i32 {
    let imported = from_le_bytes(input);
    match imported {
        FromBytes::Invalid(original) => { return 301; },
        FromBytes::Ok(vector) => {
            let mut owned = return_vector(vector);
            if len(&owned) != 0 { return 302; }
            if !missing(pop(&mut owned)) { return 303; }
            let output = return_owner(into_le_bytes(owned));
            if bytes_len(&output) != 0 { return 304; }
            if capacity(&output) != expected_capacity { return 305; }
            return 0;
        },
    }
}

fn test_reserved_empty_and_owner_returns() -> i32 {
    let empty_result = reserved_empty_result(new_bytes(), 0);
    if empty_result != 0 { return empty_result; }
    let mut owner = new_bytes();
    if !grew(try_reserve(&mut owner, 9)) { return 306; }
    let reserved_capacity = capacity(&owner);
    if reserved_capacity < 9 { return 307; }
    return reserved_empty_result(owner, reserved_capacity);
}

fn invalid_preserved(input: &[u8]) -> bool {
    let mut owner = new_bytes();
    if !grew(try_reserve(&mut owner, 19)) { return false; }
    if !grew(try_extend(&mut owner, &*input)) { return false; }
    let original_capacity = capacity(&owner);
    let imported = from_le_bytes(return_owner(owner));
    match imported {
        FromBytes::Ok(vector) => { return false; },
        FromBytes::Invalid(original) => {
            let retained = return_owner(original);
            return capacity(&retained) == original_capacity && same_bytes(&retained, &*input);
        },
    }
}

fn test_invalid_import_retains_owner() -> i32 {
    let one = [byte(255)];
    let two = [byte(0), byte(128)];
    let three = [byte(17), byte(0), byte(255)];
    let five = [byte(1), byte(2), byte(3), byte(4), byte(5)];
    if !invalid_preserved(&one) { return 401; }
    if !invalid_preserved(&two) { return 402; }
    if !invalid_preserved(&three) { return 403; }
    if !invalid_preserved(&five) { return 404; }
    return 0;
}

fn imported_values(input: ByteBuf, expected: &[i32], original_capacity: i32) -> i32 {
    let imported = from_le_bytes(input);
    match imported {
        FromBytes::Invalid(original) => { return 501; },
        FromBytes::Ok(vector) => {
            let mut owned = return_vector(vector);
            if len(&owned) != expected.len() { return 502; }
            let mut index = 0;
            while index < expected.len() {
                if !equals_value(get(&owned, index), expected[index]) { return 503; }
                index = index + 1;
            }
            while index > 0 {
                index = index - 1;
                if !equals_value(pop(&mut owned), expected[index]) { return 504; }
                if len(&owned) != index { return 505; }
            }
            if !missing(pop(&mut owned)) { return 506; }
            let output = into_le_bytes(owned);
            if bytes_len(&output) != 0 { return 507; }
            if capacity(&output) != original_capacity { return 508; }
            return 0;
        },
    }
}

fn test_signed_boundaries_fixed_bytes_and_round_trip() -> i32 {
    let values = [-2147483647 - 1, -2147483647, -65537, -256, -129, -128, -1, 0, 1, 127, 128, 255, 256, 65535, 65536, 2147483647];
    let expected = [byte(0), byte(0), byte(0), byte(128), byte(1), byte(0), byte(0), byte(128), byte(255), byte(255), byte(254), byte(255), byte(0), byte(255), byte(255), byte(255), byte(127), byte(255), byte(255), byte(255), byte(128), byte(255), byte(255), byte(255), byte(255), byte(255), byte(255), byte(255), byte(0), byte(0), byte(0), byte(0), byte(1), byte(0), byte(0), byte(0), byte(127), byte(0), byte(0), byte(0), byte(128), byte(0), byte(0), byte(0), byte(255), byte(0), byte(0), byte(0), byte(0), byte(1), byte(0), byte(0), byte(255), byte(255), byte(0), byte(0), byte(0), byte(0), byte(1), byte(0), byte(255), byte(255), byte(255), byte(127)];
    let mut vector = new_vec();
    let mut index = 0;
    while index < values.len() {
        if !grew(try_push(&mut vector, values[index])) { return 509; }
        if !equals_value(get(&vector, index), values[index]) { return 510; }
        index = index + 1;
    }
    let output = return_owner(into_le_bytes(return_vector(vector)));
    if !same_bytes(&output, &expected) { return 511; }
    let original_capacity = capacity(&output);
    let first_result = imported_values(output, &values, original_capacity);
    if first_result != 0 { return first_result; }
    let mut fixed_owner = new_bytes();
    if !grew(try_reserve(&mut fixed_owner, 65)) { return 512; }
    if !grew(try_extend(&mut fixed_owner, &expected)) { return 513; }
    let fixed_capacity = capacity(&fixed_owner);
    return imported_values(fixed_owner, &values, fixed_capacity);
}

fn main() -> i32 {
    let empty_result = test_empty_and_bounds();
    if empty_result != 0 { return empty_result; }
    let set_result = test_set_preserves_neighbors();
    if set_result != 0 { return set_result; }
    let reserved_result = test_reserved_empty_and_owner_returns();
    if reserved_result != 0 { return reserved_result; }
    let invalid_result = test_invalid_import_retains_owner();
    if invalid_result != 0 { return invalid_result; }
    let signed_result = test_signed_boundaries_fixed_bytes_and_round_trip();
    if signed_result != 0 { return signed_result; }
    return 42;
}
