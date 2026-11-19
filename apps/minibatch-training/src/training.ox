use crate::tensor::TensorResult;
use crate::tensor::model::Model;
use crate::tensor::optimizer::State;
use crate::tensor::optimizer::Config;
use crate::tensor::data::Dataset;
use crate::tensor::data::Sampler;
use crate::checkpoint::pipeline::Recovery;
use crate::options::Options;

fn saved_ok(saved: TensorResult<()>) -> bool {
    match saved { TensorResult::Ok(done) => { return true; }, TensorResult::Err(error) => { return false; } }
}
fn save_to(model: &Model, state: &State, data: &Dataset, sampler: &Sampler, options: &Options, slot: i32) -> bool {
    if slot == 1 { return saved_ok(crate::checkpoint::pipeline::save(&*options.second, &*model, &*state, &*data, &*sampler)); }
    return saved_ok(crate::checkpoint::pipeline::save(&*options.first, &*model, &*state, &*data, &*sampler));
}
fn fit(created: Model, created_state: State, data: Dataset, created_sampler: Sampler, previous_slot: i32, options: &Options) -> i32 {
    let mut model = created;
    let mut state = created_state;
    let mut sampler = created_sampler;
    let mut slot = previous_slot;
    for index in 0..options.steps {
        let updated = crate::tensor::data::train_next(&mut model, &mut state, &data, &mut sampler);
        match updated { TensorResult::Ok(number) => {}, TensorResult::Err(error) => { return crate::report::failure(); } }
        if (index + 1) % 25 == 0 || index + 1 == options.steps {
            let next_slot = 1 - slot;
            if !save_to(&model, &state, &data, &sampler, &*options, next_slot) { return crate::report::failure(); }
            slot = next_slot;
        }
    }
    let measured = crate::tensor::data::loss(&model, &data);
    match measured {
        TensorResult::Ok(number) => {
            let updates = crate::tensor::data::batch_count(&sampler);
            if slot == 1 { return crate::report::summary(updates, number, &*options.second); }
            return crate::report::summary(updates, number, &*options.first);
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
fn prepared(model: Model, data: Dataset, config: Config, options: &Options) -> i32 {
    let initialized = crate::tensor::optimizer::initialize(1, crate::tensor::model::parameter_elements(&model), config);
    match initialized {
        TensorResult::Ok(state) => {
            let sampling = crate::tensor::data::initialize_sampler(&data, options.size, true, options.seed);
            match sampling {
                TensorResult::Ok(sampler) => {
                    if !save_to(&model, &state, &data, &sampler, &*options, 0) { return crate::report::failure(); }
                    return fit(model, state, data, sampler, 0, &*options);
                },
                TensorResult::Err(error) => { return crate::report::failure(); }
            }
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
fn fresh_model(data: Dataset, options: &Options) -> i32 {
    let features = crate::tensor::data::feature_count(&data);
    let targets = crate::tensor::data::target_count(&data);
    let made = crate::tensor::model::initialize(features, 8, targets, 42u32);
    match made {
        TensorResult::Ok(model) => {
            let configured = configuration(options.algorithm);
            match configured { TensorResult::Ok(config) => { return prepared(model, data, config, &*options); }, TensorResult::Err(error) => { return crate::report::failure(); } }
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
fn fresh(options: &Options) -> i32 {
    let first = vacant(&*options.first);
    let second = vacant(&*options.second);
    if first < 0 || second < 0 { return crate::report::failure(); }
    if first > 0 || second > 0 { return crate::report::existing(); }
    let loaded = crate::checkpoint::pipeline::load_dataset(&*options.source);
    match loaded { TensorResult::Ok(data) => { return fresh_model(data, &*options); }, TensorResult::Err(error) => { return crate::report::failure(); } }
}
pub fn execute(options: &Options) -> i32 {
    if options.mode == 1 { return fresh(&*options); }
    let loaded = crate::checkpoint::pipeline::load_latest(&*options.first, &*options.second);
    match loaded {
        TensorResult::Ok(recovery) => {
            let Recovery { model: selected, optimizer: state, dataset: data, sampler: sampler, slot: slot } = recovery;
            return fit(selected, state, data, sampler, slot, &*options);
        },
        TensorResult::Err(error) => { return crate::report::failure(); }
    }
}
