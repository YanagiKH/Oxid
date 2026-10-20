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
use vec::try_push;
use vec::from_le_bytes;
use vec::into_le_bytes;
use algorithms::heapsort;
use algorithms::lower_bound;
use algorithms::LowerBound;

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn append_values(vector: &mut I32Vec, values: &[i32]) -> bool {
    let mut index = 0;
    while index < values.len() {
        if !grew(try_push(&mut *vector, values[index])) { return false; }
        index = index + 1;
    }
    return true;
}

fn equals_value(element: Element, expected: i32) -> bool {
    match element {
        Element::Value(actual) => { return actual == expected; },
        Element::Missing => { return false; },
    }
}

fn same_values(vector: &I32Vec, expected: &[i32]) -> bool {
    if len(&*vector) != expected.len() { return false; }
    let mut index = 0;
    while index < expected.len() {
        if !equals_value(get(&*vector, index), expected[index]) { return false; }
        index = index + 1;
    }
    return true;
}

fn lower_is(vector: &I32Vec, key: i32, expected: i32) -> bool {
    let result = lower_bound(&*vector, key);
    match result {
        LowerBound::Index(actual) => { return actual == expected; },
        LowerBound::ContractViolation => { return false; },
    }
}

fn same_bytes(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() { return false; }
    let mut index = 0;
    while index < expected.len() {
        if actual[index] != expected[index] { return false; }
        index = index + 1;
    }
    return true;
}

fn test_sort_empty_and_singleton() -> i32 {
    let mut empty = new_vec();
    if !heapsort(&mut empty) { return 101; }
    if len(&empty) != 0 { return 102; }

    let one_value = [7];
    let mut one = new_vec();
    if !append_values(&mut one, &one_value) { return 103; }
    if !heapsort(&mut one) { return 104; }
    if !same_values(&one, &one_value) { return 105; }

    let minimum = -2147483647 - 1;
    let minimum_value = [minimum];
    let mut minimum_vector = new_vec();
    if !append_values(&mut minimum_vector, &minimum_value) { return 106; }
    if !heapsort(&mut minimum_vector) { return 107; }
    if !same_values(&minimum_vector, &minimum_value) { return 108; }

    let maximum = [2147483647];
    let mut maximum_vector = new_vec();
    if !append_values(&mut maximum_vector, &maximum) { return 109; }
    if !heapsort(&mut maximum_vector) { return 110; }
    if !same_values(&maximum_vector, &maximum) { return 111; }
    return 0;
}

fn test_sort_pairs_and_odd_even_heaps() -> i32 {
    let pair = [2, 1];
    let pair_sorted = [1, 2];
    let mut pair_vector = new_vec();
    if !append_values(&mut pair_vector, &pair) { return 201; }
    if !heapsort(&mut pair_vector) { return 202; }
    if !same_values(&pair_vector, &pair_sorted) { return 203; }

    let odd = [3, 1, 2];
    let odd_sorted = [1, 2, 3];
    let mut odd_vector = new_vec();
    if !append_values(&mut odd_vector, &odd) { return 204; }
    if !heapsort(&mut odd_vector) { return 205; }
    if !same_values(&odd_vector, &odd_sorted) { return 206; }

    let even = [4, 1, 3, 2];
    let even_sorted = [1, 2, 3, 4];
    let mut even_vector = new_vec();
    if !append_values(&mut even_vector, &even) { return 207; }
    if !heapsort(&mut even_vector) { return 208; }
    if !same_values(&even_vector, &even_sorted) { return 209; }

    let descending = [5, 4, 3, 2, 1];
    let descending_sorted = [1, 2, 3, 4, 5];
    let mut descending_vector = new_vec();
    if !append_values(&mut descending_vector, &descending) { return 210; }
    if !heapsort(&mut descending_vector) { return 211; }
    if !same_values(&descending_vector, &descending_sorted) { return 212; }
    return 0;
}

fn test_sort_signed_extremes_and_duplicates() -> i32 {
    let minimum = -2147483647 - 1;
    let values = [2147483647, 0, minimum, -1, 2147483647, minimum, 0, 1];
    let expected = [minimum, minimum, -1, 0, 0, 1, 2147483647, 2147483647];
    let mut vector = new_vec();
    if !append_values(&mut vector, &values) { return 301; }
    if !heapsort(&mut vector) { return 302; }
    if !same_values(&vector, &expected) { return 303; }
    return 0;
}

fn test_sort_sorted_reverse_equal_and_idempotent() -> i32 {
    let already_sorted = [-3, -2, -1, 0, 1, 2, 3];
    let mut sorted_vector = new_vec();
    if !append_values(&mut sorted_vector, &already_sorted) { return 401; }
    if !heapsort(&mut sorted_vector) { return 402; }
    if !same_values(&sorted_vector, &already_sorted) { return 403; }
    if !heapsort(&mut sorted_vector) { return 404; }
    if !same_values(&sorted_vector, &already_sorted) { return 405; }

    let descending = [3, 2, 1, 0, -1, -2, -3];
    let ascending = [-3, -2, -1, 0, 1, 2, 3];
    let mut reverse_vector = new_vec();
    if !append_values(&mut reverse_vector, &descending) { return 406; }
    if !heapsort(&mut reverse_vector) { return 407; }
    if !same_values(&reverse_vector, &ascending) { return 408; }
    if !heapsort(&mut reverse_vector) { return 409; }
    if !same_values(&reverse_vector, &ascending) { return 410; }

    let equal = [4, 4, 4, 4, 4];
    let mut equal_vector = new_vec();
    if !append_values(&mut equal_vector, &equal) { return 411; }
    if !heapsort(&mut equal_vector) { return 412; }
    if !same_values(&equal_vector, &equal) { return 413; }
    if !heapsort(&mut equal_vector) { return 414; }
    if !same_values(&equal_vector, &equal) { return 415; }

    let late_inversion = [-3, -2, -1, 0, 1, 3, 2];
    let late_sorted = [-3, -2, -1, 0, 1, 2, 3];
    let mut late_vector = new_vec();
    if !append_values(&mut late_vector, &late_inversion) { return 416; }
    if !heapsort(&mut late_vector) { return 417; }
    if !same_values(&late_vector, &late_sorted) { return 418; }
    if !heapsort(&mut late_vector) { return 419; }
    if !same_values(&late_vector, &late_sorted) { return 420; }

    let minimum = -2147483647 - 1;
    let sorted_extremes = [minimum, minimum, -1, 0, 0, 2147483647, 2147483647];
    let mut extremes_vector = new_vec();
    if !append_values(&mut extremes_vector, &sorted_extremes) { return 421; }
    if !heapsort(&mut extremes_vector) { return 422; }
    if !same_values(&extremes_vector, &sorted_extremes) { return 423; }
    if !heapsort(&mut extremes_vector) { return 424; }
    if !same_values(&extremes_vector, &sorted_extremes) { return 425; }
    return 0;
}

fn test_sort_preserves_capacity_and_owner() -> i32 {
    let mut owner = new_bytes();
    if !grew(try_reserve(&mut owner, 37)) { return 501; }
    let fixed_input = [byte(2), byte(0), byte(0), byte(0), byte(255), byte(255), byte(255), byte(255), byte(2), byte(0), byte(0), byte(0)];
    if !grew(try_extend(&mut owner, &fixed_input)) { return 502; }
    let original_capacity = capacity(&owner);
    if original_capacity < 37 { return 503; }

    let imported = from_le_bytes(owner);
    match imported {
        FromBytes::Invalid(original) => { return 504; },
        FromBytes::Ok(vector) => {
            let mut sortable = vector;
            if len(&sortable) != 3 { return 505; }
            if !heapsort(&mut sortable) { return 506; }
            let expected_values = [-1, 2, 2];
            if !same_values(&sortable, &expected_values) { return 507; }
            let output = into_le_bytes(sortable);
            let fixed_output = [byte(255), byte(255), byte(255), byte(255), byte(2), byte(0), byte(0), byte(0), byte(2), byte(0), byte(0), byte(0)];
            if !same_bytes(&output, &fixed_output) { return 508; }
            if bytes_len(&output) != 12 { return 509; }
            if capacity(&output) != original_capacity { return 510; }
        },
    }

    let mut empty_owner = new_bytes();
    if !grew(try_reserve(&mut empty_owner, 37)) { return 511; }
    let empty_capacity = capacity(&empty_owner);
    if empty_capacity < 37 { return 512; }
    let imported_empty = from_le_bytes(empty_owner);
    match imported_empty {
        FromBytes::Invalid(original) => { return 513; },
        FromBytes::Ok(empty_vector) => {
            let mut sortable_empty = empty_vector;
            if len(&sortable_empty) != 0 { return 514; }
            if !heapsort(&mut sortable_empty) { return 515; }
            if len(&sortable_empty) != 0 { return 516; }
            let empty_output = into_le_bytes(sortable_empty);
            if bytes_len(&empty_output) != 0 { return 517; }
            if capacity(&empty_output) != empty_capacity { return 518; }
        },
    }
    return 0;
}

fn test_lower_bound_empty_singleton_and_edges() -> i32 {
    let minimum = -2147483647 - 1;
    let maximum = 2147483647;
    let empty = new_vec();
    if !lower_is(&empty, minimum, 0) { return 601; }
    if !lower_is(&empty, 0, 0) { return 602; }
    if !lower_is(&empty, maximum, 0) { return 603; }

    let singleton_values = [7];
    let mut singleton = new_vec();
    if !append_values(&mut singleton, &singleton_values) { return 604; }
    if !lower_is(&singleton, 6, 0) { return 605; }
    if !lower_is(&singleton, 7, 0) { return 606; }
    if !lower_is(&singleton, 8, 1) { return 607; }

    let extremes = [minimum, maximum];
    let mut extreme_vector = new_vec();
    if !append_values(&mut extreme_vector, &extremes) { return 608; }
    if !lower_is(&extreme_vector, minimum, 0) { return 609; }
    if !lower_is(&extreme_vector, 0, 1) { return 610; }
    if !lower_is(&extreme_vector, maximum, 1) { return 611; }
    return 0;
}

fn test_lower_bound_duplicates_and_gaps() -> i32 {
    let values = [-5, -1, -1, -1, 2, 2, 9];
    let mut vector = new_vec();
    if !append_values(&mut vector, &values) { return 701; }
    if !lower_is(&vector, -6, 0) { return 702; }
    if !lower_is(&vector, -5, 0) { return 703; }
    if !lower_is(&vector, -4, 1) { return 704; }
    if !lower_is(&vector, -1, 1) { return 705; }
    if !lower_is(&vector, 0, 4) { return 706; }
    if !lower_is(&vector, 2, 4) { return 707; }
    if !lower_is(&vector, 3, 6) { return 708; }
    if !lower_is(&vector, 9, 6) { return 709; }
    if !lower_is(&vector, 10, 7) { return 710; }

    let equal_values = [4, 4, 4];
    let mut equal_vector = new_vec();
    if !append_values(&mut equal_vector, &equal_values) { return 711; }
    if !lower_is(&equal_vector, 3, 0) { return 712; }
    if !lower_is(&equal_vector, 4, 0) { return 713; }
    if !lower_is(&equal_vector, 5, 3) { return 714; }
    return 0;
}

fn test_lower_bound_read_only() -> i32 {
    let minimum = -2147483647 - 1;
    let maximum = 2147483647;
    let values = [minimum, -1, 0, 0, maximum];
    let mut initial = new_vec();
    if !append_values(&mut initial, &values) { return 801; }
    let initial_owner = into_le_bytes(initial);
    let original_capacity = capacity(&initial_owner);
    let fixed_bytes = [byte(0), byte(0), byte(0), byte(128), byte(255), byte(255), byte(255), byte(255), byte(0), byte(0), byte(0), byte(0), byte(0), byte(0), byte(0), byte(0), byte(255), byte(255), byte(255), byte(127)];
    if !same_bytes(&initial_owner, &fixed_bytes) { return 802; }
    if bytes_len(&initial_owner) != 20 { return 803; }

    let imported = from_le_bytes(initial_owner);
    match imported {
        FromBytes::Invalid(original) => { return 804; },
        FromBytes::Ok(vector) => {
            if len(&vector) != 5 { return 805; }
            if !lower_is(&vector, minimum, 0) { return 806; }
            if !lower_is(&vector, -1, 1) { return 807; }
            if !lower_is(&vector, 0, 2) { return 808; }
            if !lower_is(&vector, 1, 4) { return 809; }
            if !lower_is(&vector, maximum, 4) { return 810; }
            if !lower_is(&vector, minimum, 0) { return 811; }
            if !lower_is(&vector, -1, 1) { return 812; }
            if !lower_is(&vector, 0, 2) { return 813; }
            if !lower_is(&vector, 1, 4) { return 814; }
            if !lower_is(&vector, maximum, 4) { return 815; }
            if !same_values(&vector, &values) { return 816; }
            let output = into_le_bytes(vector);
            if !same_bytes(&output, &fixed_bytes) { return 817; }
            if bytes_len(&output) != 20 { return 818; }
            if capacity(&output) != original_capacity { return 819; }
        },
    }
    return 0;
}

fn test_sort_then_search_client_composition() -> i32 {
    let values = [3, -2, 3, 1, -2];
    let sorted = [-2, -2, 1, 3, 3];
    let mut vector = new_vec();
    if !append_values(&mut vector, &values) { return 901; }
    if !heapsort(&mut vector) { return 902; }
    if !same_values(&vector, &sorted) { return 903; }
    if !lower_is(&vector, -3, 0) { return 904; }
    if !lower_is(&vector, -2, 0) { return 905; }
    if !lower_is(&vector, 0, 2) { return 906; }
    if !lower_is(&vector, 1, 2) { return 907; }
    if !lower_is(&vector, 2, 3) { return 908; }
    if !lower_is(&vector, 3, 3) { return 909; }
    if !lower_is(&vector, 4, 5) { return 910; }
    return 0;
}

fn main() -> i32 {
    let empty_result = test_sort_empty_and_singleton();
    if empty_result != 0 { return empty_result; }
    let heaps_result = test_sort_pairs_and_odd_even_heaps();
    if heaps_result != 0 { return heaps_result; }
    let extremes_result = test_sort_signed_extremes_and_duplicates();
    if extremes_result != 0 { return extremes_result; }
    let idempotence_result = test_sort_sorted_reverse_equal_and_idempotent();
    if idempotence_result != 0 { return idempotence_result; }
    let owner_result = test_sort_preserves_capacity_and_owner();
    if owner_result != 0 { return owner_result; }
    let lower_edges_result = test_lower_bound_empty_singleton_and_edges();
    if lower_edges_result != 0 { return lower_edges_result; }
    let lower_duplicates_result = test_lower_bound_duplicates_and_gaps();
    if lower_duplicates_result != 0 { return lower_duplicates_result; }
    let read_only_result = test_lower_bound_read_only();
    if read_only_result != 0 { return read_only_result; }
    let composition_result = test_sort_then_search_client_composition();
    if composition_result != 0 { return composition_result; }
    return 42;
}
