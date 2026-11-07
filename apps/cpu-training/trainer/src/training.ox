use crate::tensor::TensorResult;
use crate::tensor::Tensor;
use crate::tensor::model::Model;
use crate::checkpoint::training::Recovery;
use crate::tensor::optimizer::State;
use crate::tensor::optimizer::Config;
use crate::options::Options;

fn saved_ok(saved: TensorResult<()>) -> bool {
    match saved { TensorResult::Ok(done) => { return true; }, TensorResult::Err(error) => { return false; } }
}
fn save_to(model: &Model, state: &State, options: &Options, slot: i32, legacy: bool) -> bool {
    if !legacy {
        if slot == 1 { return saved_ok(crate::checkpoint::training::save(&*options.second, &*model, &*state)); }
        return saved_ok(crate::checkpoint::training::save(&*options.first, &*model, &*state));
    }
    if slot == 1 { return saved_ok(crate::checkpoint::save(&*options.second, &*model)); }
    return saved_ok(crate::checkpoint::save(&*options.first, &*model));
}
fn fit(model: &mut Model, state: &mut State, input: &Tensor, target: &Tensor, options: &Options, previous_slot: i32, legacy: bool) -> i32 {
    let mut slot = previous_slot;
    for index in 0..options.steps {
        let updated = crate::tensor::model::train_with_optimizer(&mut *model, &mut *state, &*input, &*target);
        match updated { TensorResult::Ok(number) => {}, TensorResult::Err(error) => { return crate::report::failure(); } }
        if (index + 1) % 25 == 0 || index + 1 == options.steps {
            let next_slot = 1 - slot;
            if !save_to(&*model, &*state, &*options, next_slot, legacy) { return crate::report::failure(); } slot = next_slot;
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
fn run(created: Model, created_state: State, slot: i32, legacy: bool, options: &Options) -> i32 {
    let mut model = created;
    let mut state = created_state;
    let x = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0]; let y = [0.0, 1.0, 1.0, 0.0];
    let made = crate::tensor::from_slice(4, 2, &x);
    match made {
        TensorResult::Ok(input) => {
            let expected = crate::tensor::from_slice(4, 1, &y);
            match expected { TensorResult::Ok(target) => { return fit(&mut model, &mut state, &input, &target, &*options, slot, legacy); }, TensorResult::Err(error) => { return crate::report::failure(); } }
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
fn vacant(path: &[u8]) -> i32 {
    let found = crate::checkpoint::exists(&*path);
    match found { TensorResult::Ok(present) => { if present { return 1; } return 0; }, TensorResult::Err(error) => { return -1; } }
}
fn configuration(algorithm: i32) -> TensorResult<Config>{
    if algorithm == 1 { return crate::tensor::optimizer::adam(0.01, 0.9, 0.999, 1e-8); }
    if algorithm == 2 { return crate::tensor::optimizer::adamw(0.01, 0.9, 0.999, 1e-8, 0.01); }
    return crate::tensor::optimizer::sgd(0.05);
}
fn prepared(model: Model, config: Config, options: &Options) -> i32 {
    let initialized = crate::tensor::optimizer::initialize(1, crate::tensor::model::parameter_elements(&model), config);
    match initialized {
        TensorResult::Ok(state) => { let legacy = options.algorithm == 0; if !save_to(&model, &state, &*options, 0, legacy) { return crate::report::failure(); } return run(model, state, 0, legacy, &*options); },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
fn fresh(options: &Options) -> i32 {
    let first = vacant(&*options.first); let second = vacant(&*options.second);
    if first < 0 || second < 0 { return crate::report::failure(); } if first > 0 || second > 0 { return crate::report::existing(); }
    let made = crate::tensor::model::initialize(2, 8, 1, 42u32);
    match made {
        TensorResult::Ok(model) => {
            let configured = configuration(options.algorithm);
            match configured { TensorResult::Ok(config) => { return prepared(model, config, &*options); }, TensorResult::Err(error) => { return crate::report::failure(); } }
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
pub fn execute(options: &Options) -> i32 {
    if options.mode == 1 { return fresh(&*options); }
    let loaded = crate::checkpoint::training::load_latest(&*options.first, &*options.second);
    match loaded {
        TensorResult::Ok(recovery) => { let Recovery { model: selected, optimizer: state, slot: slot, legacy: old_format } = recovery; return run(selected, state, slot, old_format, &*options); },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
