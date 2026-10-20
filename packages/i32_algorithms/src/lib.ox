use vec::I32Vec;
use vec::Element;
use vec::len;
use vec::get;
use vec::set;

pub enum LowerBound {
    Index(i32),
    ContractViolation
}

// Read both values before writing either destination; propagate every status.
fn swap(values: &mut I32Vec, first: i32, second: i32) -> bool {
    let first_value = get(&*values, first);
    match first_value {
        Element::Missing => { return false; },
        Element::Value(left) => {
            let second_value = get(&*values, second);
            match second_value {
                Element::Missing => { return false; },
                Element::Value(right) => {
                    if !set(&mut *values, first, right) { return false; }
                    if !set(&mut *values, second, left) { return false; }
                    return true;
                },
            }
        },
    }
}

// Restore a max heap in the half-open interval [0, end).
fn sift_down(values: &mut I32Vec, start: i32, end: i32) -> bool {
    let mut root = start;
    while end >= 2 && root <= (end - 2) / 2 {
        // The parent bound proves this left child is in [0, end).
        let left = 2 * root + 1;
        let left_result = get(&*values, left);
        match left_result {
            Element::Missing => { return false; },
            Element::Value(left_value) => {
                let mut selected = left;
                let mut selected_value = left_value;
                let right = left + 1;
                if right < end {
                    let right_result = get(&*values, right);
                    match right_result {
                        Element::Missing => { return false; },
                        Element::Value(value) => {
                            if value > selected_value {
                                selected = right;
                                selected_value = value;
                            }
                        },
                    }
                }
                let root_result = get(&*values, root);
                match root_result {
                    Element::Missing => { return false; },
                    Element::Value(root_value) => {
                        if selected_value <= root_value { return true; }
                        // Both original values are read before either write.
                        if !set(&mut *values, root, selected_value) { return false; }
                        if !set(&mut *values, selected, root_value) { return false; }
                        root = selected;
                    },
                }
            },
        }
    }
    return true;
}

// Sort all elements ascending by signed i32 order, in place and without allocation.
pub fn heapsort(values: &mut I32Vec) -> bool {
    let count = len(&*values);
    if count < 2 { return true; }

    // Return early only after reading the entire nondecreasing input.
    let first = get(&*values, 0);
    match first {
        Element::Missing => { return false; },
        Element::Value(first_value) => {
            let mut previous = first_value;
            let mut index = 1;
            while index < count {
                let next = get(&*values, index);
                match next {
                    Element::Missing => { return false; },
                    Element::Value(current) => {
                        if previous > current { break; }
                        previous = current;
                    },
                }
                index = index + 1;
            }
            if index == count { return true; }
        },
    }

    // Start one past the last possible parent and decrement before each sift.
    let mut parent = count / 2;
    while parent > 0 {
        parent = parent - 1;
        if !sift_down(&mut *values, parent, count) { return false; }
    }

    // Shrink the heap before moving its maximum to the sorted suffix.
    let mut end = count;
    while end > 1 {
        end = end - 1;
        if !swap(&mut *values, 0, end) { return false; }
        if !sift_down(&mut *values, 0, end) { return false; }
    }
    return true;
}

// Find the first element >= key in an already ascending vector.
pub fn lower_bound(values: &I32Vec, key: i32) -> LowerBound {
    let count = len(&*values);
    let mut lo = 0;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let result = get(&*values, mid);
        match result {
            Element::Missing => { return LowerBound::ContractViolation; },
            Element::Value(value) => {
                if value < key {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            },
        }
    }
    return LowerBound::Index(lo);
}
