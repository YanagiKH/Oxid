mod options;
mod report;
mod training;
use crate::tensor::TensorResult;
fn main() -> i32 {
    if crate::options::help_requested() { return crate::report::help(); }
    let parsed = crate::options::parse(); match parsed {
        TensorResult::Ok(options) => { let trained = crate::training::run(&options.path, options.epochs, options.size); match trained { TensorResult::Ok(summary) => { return crate::report::summary(&summary); }, TensorResult::Err(error) => { return crate::report::failure(); }, } },
        TensorResult::Err(error) => { crate::report::help(); return 2; }, }
}
