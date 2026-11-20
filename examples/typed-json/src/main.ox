use crate::json::Validation;
use crate::json::validate;

fn main() -> i32 {
    let document = b"{\"city\":\"東京\",\"temperatures\":[12.5,-1],\"available\":true}";
    let result = validate(&document);
    match result {
        Validation::Valid => { return 1; },
        Validation::Syntax(offset) => { return 0; },
        Validation::InvalidUtf8(offset) => { return 0; },
        Validation::CapacityOverflow => { return 0; },
        Validation::AllocationFailed => { return 0; },
        Validation::MemoryLimit => { return 0; },
    }
}
