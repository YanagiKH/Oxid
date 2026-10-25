use crate::tensor::TensorResult;
use crate::tensor::Tensor;
use crate::tensor::model::Model;
use crate::checkpoint::Recovery;
use crate::options::Options;

fn saved_ok(saved: TensorResult<()>) -> bool {
    match saved { TensorResult::Ok(done) => { return true; }, TensorResult::Err(error) => { return false; } }
}
fn save_to(model: &Model, options: &Options, slot: i32) -> bool {
    if slot == 1 { return saved_ok(crate::checkpoint::save(&*options.second, &*model)); }
    return saved_ok(crate::checkpoint::save(&*options.first, &*model));
}
fn fit(model: &mut Model, input: &Tensor, target: &Tensor, options: &Options, previous_slot: i32) -> i32 {
    let mut slot = previous_slot;
    for index in 0..options.steps {
        let updated = crate::tensor::model::train_step(&mut *model, &*input, &*target, 0.05);
        match updated { TensorResult::Ok(number) => {}, TensorResult::Err(error) => { return crate::report::failure(); } }
        if (index + 1) % 25 == 0 || index + 1 == options.steps {
            let next_slot = 1 - slot;
            if !save_to(&*model, &*options, next_slot) { return crate::report::failure(); } slot = next_slot;
        }
    }
    let measured = crate::tensor::model::loss(&*model, &*input, &*target);
    match measured {
        TensorResult::Ok(number) => {
            let updates = crate::tensor::model::update_count(&*model);
            if slot == 1 { return crate::report::summary(updates, number, &*options.second); } return crate::report::summary(updates, number, &*options.first);
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
fn run(created: Model, slot: i32, options: &Options) -> i32 {
    let mut model = created;
    let x = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0]; let y = [0.0, 1.0, 1.0, 0.0];
    let made = crate::tensor::from_slice(4, 2, &x);
    match made {
        TensorResult::Ok(input) => {
            let expected = crate::tensor::from_slice(4, 1, &y);
            match expected { TensorResult::Ok(target) => { return fit(&mut model, &input, &target, &*options, slot); }, TensorResult::Err(error) => { return crate::report::failure(); } }
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
fn vacant(path: &[u8]) -> i32 {
    let found = crate::checkpoint::exists(&*path);
    match found { TensorResult::Ok(present) => { if present { return 1; } return 0; }, TensorResult::Err(error) => { return -1; } }
}
fn fresh(options: &Options) -> i32 {
    let first = vacant(&*options.first); let second = vacant(&*options.second);
    if first < 0 || second < 0 { return crate::report::failure(); } if first > 0 || second > 0 { return crate::report::existing(); }
    let made = crate::tensor::model::initialize(2, 8, 1, 42u32);
    match made {
        TensorResult::Ok(model) => { if !save_to(&model, &*options, 0) { return crate::report::failure(); } return run(model, 0, &*options); },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
pub fn execute(options: &Options) -> i32 {
    if options.mode == 1 { return fresh(&*options); }
    let loaded = crate::checkpoint::load_latest_with_slot(&*options.first, &*options.second);
    match loaded {
        TensorResult::Ok(recovery) => { let Recovery { model: selected, slot: slot } = recovery; return run(selected, slot, &*options); },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
