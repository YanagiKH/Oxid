mod checkpoint;
mod options;
mod report;
mod training;
use crate::tensor::TensorResult;
use std::process::enable_termination_notifications;
use std::process::termination_requested;
fn main() -> i32 {
    if crate::options::help_requested() { return crate::report::help(); }
    let parsed = crate::options::parse(); match parsed {
        TensorResult::Ok(options) => {
            enable_termination_notifications();
            let trained = crate::training::run(&options);
            if termination_requested() { return 130; }
            match trained {
                TensorResult::Ok(summary) => { return crate::report::summary(&summary); },
                TensorResult::Err(error) => { return crate::report::failure(); },
            }
        },
        TensorResult::Err(error) => { crate::report::help(); return 2; }, }
}
