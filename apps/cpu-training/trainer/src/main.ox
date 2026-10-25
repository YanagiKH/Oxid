mod options;
mod report;
mod training;
use crate::tensor::TensorResult;
use std::env::arg_count;
fn main() -> i32 {
    if arg_count() == 1 || crate::options::help_requested() { return crate::report::help(); }
    let parsed = crate::options::parse();
    match parsed { TensorResult::Ok(options) => { return crate::training::execute(&options); }, TensorResult::Err(error) => { crate::report::help(); return 2; } }
}
