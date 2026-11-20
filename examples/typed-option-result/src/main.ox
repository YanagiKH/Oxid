use crate::types::option::Option;
use crate::types::option::ok_or;
use crate::types::result::Result;

fn requested_port(valid: bool) -> Option<i32> {
    if valid { return Option::<i32>::Some(8080); }
    return Option::<i32>::None;
}

fn main() -> i32 {
    let port = ok_or(requested_port(true), 1);
    match port {
        Result::Ok(number) => { return number; },
        Result::Err(code) => { return 0 - code; },
    }
}
