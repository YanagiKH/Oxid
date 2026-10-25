use crate::Tensor;
use crate::TensorResult;
use crate::Error;
use crate::graph::Graph;
use crate::graph::Variable;
pub mod checkpoint;

pub struct Model {
    weights1: Tensor,
    bias1: Tensor,
    weights2: Tensor,
    bias2: Tensor,
    rng: u32,
    updates: u64
}

pub struct Random { state: u32 }

struct TrainingTape { weights1: Variable, bias1: Variable, weights2: Variable, bias2: Variable, loss: Variable }

pub fn update_count(model: &Model) -> u64 { return model.updates; }

pub fn parameter(model: &Model, index: i32) -> TensorResult<Tensor>{
    if index == 0 { return crate::duplicate(&*model.weights1); }
    if index == 1 { return crate::duplicate(&*model.bias1); }
    if index == 2 { return crate::duplicate(&*model.weights2); }
    if index == 3 { return crate::duplicate(&*model.bias2); }
    return TensorResult::<Tensor>:: Err(Error::Bounds);
}

fn finite(number: f64) -> bool { return ((number.to_bits() >> 52) & 2047u64) != 2047u64; }

pub fn random(seed: u32) -> Random {
    let mut state = seed;
    if state == 0u32 { state = 1831565813u32; }
    return Random { state: state };
}

pub fn next_random(state: &mut Random) -> u32 {
    let mut bits = state.state;
    bits = bits ^ (bits << 13);
    bits = bits ^ (bits >> 17);
    bits = bits ^ (bits << 5);
    state.state = bits;
    return bits;
}

fn root(number: f64) -> f64 {
    let mut estimate = 1.0;
    for iteration in 0..32 { estimate = 0.5 * (estimate + number / estimate); }
    return estimate;
}

fn initialize_values(bytes: &mut [u8], state: &mut Random, fan_in: i32) -> () {
    let scale = root(6.0 / fan_in.to_f64());
    for element in 0..bytes.len() / 8 {
        let draw = next_random(&mut *state) >> 8;
        let number = (draw.to_f64() / 16777216.0 * 2.0 - 1.0) * scale;
        crate::codec::store(&mut *bytes, element, number);
    }
    return;
}

pub fn initialize(inputs: i32, hidden: i32, outputs: i32, seed: u32) -> TensorResult<Model>{
    if inputs <= 0 || hidden <= 0 || outputs <= 0 { return TensorResult::<Model>:: Err(Error::NegativeShape); }
    let mut state = random(seed);
    let mut weights1 = crate::empty();
    let mut bias1 = crate::empty();
    let mut weights2 = crate::empty();
    let mut bias2 = crate::empty();
    let first = crate::filled(inputs, hidden, 0.0);
    match first { TensorResult::Ok(tensor) => { weights1 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let second = crate::filled(1, hidden, 0.0);
    match second { TensorResult::Ok(tensor) => { bias1 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let third = crate::filled(hidden, outputs, 0.0);
    match third { TensorResult::Ok(tensor) => { weights2 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    let fourth = crate::filled(1, outputs, 0.0);
    match fourth { TensorResult::Ok(tensor) => { bias2 = tensor; }, TensorResult::Err(error) => { return TensorResult::<Model>:: Err(error); } }
    initialize_values(&mut weights1.data, &mut state, inputs);
    initialize_values(&mut weights2.data, &mut state, hidden);
    return TensorResult::Ok(Model { weights1: weights1, bias1: bias1, weights2: weights2, bias2: bias2, rng: state.state, updates: 0u64 });
}

fn dense(input: &Tensor, weights: &Tensor, bias: &Tensor) -> TensorResult<Tensor>{
    let product = crate::matmul(&*input, &*weights);
    match product {
        TensorResult::Ok(tensor) => { return crate::add_rows(&tensor, &*bias); },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
}

pub fn predict(model: &Model, input: &Tensor) -> TensorResult<Tensor>{
    let first = dense(&*input, &*model.weights1, &*model.bias1);
    match first {
        TensorResult::Ok(tensor) => {
            let activated = crate::relu(&tensor);
            match activated {
                TensorResult::Ok(hidden) => { return dense(&hidden, &*model.weights2, &*model.bias2); },
                TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
}

pub fn loss(model: &Model, input: &Tensor, expected: &Tensor) -> TensorResult<f64>{
    let predicted = predict(&*model, &*input);
    match predicted {
        TensorResult::Ok(tensor) => { return crate::mean_squared_error(&tensor, &*expected); },
        TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }
    }
}

fn validate_updates(model: &Model, graph: &Graph, tape: &TrainingTape, rate: f64) -> TensorResult<()>{
    let first = crate::graph::validate_sgd_update(&*model.weights1, &*graph, &*tape.weights1, rate);
    match first { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let second = crate::graph::validate_sgd_update(&*model.bias1, &*graph, &*tape.bias1, rate);
    match second { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let third = crate::graph::validate_sgd_update(&*model.weights2, &*graph, &*tape.weights2, rate);
    match third { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    return crate::graph::validate_sgd_update(&*model.bias2, &*graph, &*tape.bias2, rate);
}

fn commit_updates(model: &mut Model, graph: &Graph, tape: &TrainingTape, rate: f64) -> TensorResult<()>{
    let first = crate::graph::sgd_update(&mut *model.weights1, &*graph, &*tape.weights1, rate);
    match first { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let second = crate::graph::sgd_update(&mut *model.bias1, &*graph, &*tape.bias1, rate);
    match second { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let third = crate::graph::sgd_update(&mut *model.weights2, &*graph, &*tape.weights2, rate);
    match third { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    return crate::graph::sgd_update(&mut *model.bias2, &*graph, &*tape.bias2, rate);
}

pub fn train_step(model: &mut Model, input: &Tensor, expected: &Tensor, rate: f64) -> TensorResult<f64>{
    if model.updates == 18446744073709551615u64 { return TensorResult::<f64>:: Err(Error::StepOverflow); }
    if !finite(rate) || rate < 0.0 { return TensorResult::<f64>:: Err(Error::InvalidRate); }
    let mut graph = crate::graph::create();
    let tape_result = training_tape(&mut graph, &*model, &*input, &*expected);
    match tape_result {
        TensorResult::Ok(tape_value) => {
            let tape = tape_value;
            let backward = crate::graph::backward(&mut graph, &tape.loss);
            match backward { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); } }
            let mut loss_number = 0.0;
            let read = crate::graph::value_at(&graph, &tape.loss, 0, 0);
            match read { TensorResult::Ok(number) => { loss_number = number; }, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); } }
            if !finite(loss_number) { return TensorResult::<f64>:: Err(Error::NonFinite); }
            let checked = validate_updates(&*model, &graph, &tape, rate);
            match checked { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); } }
            let committed = commit_updates(&mut *model, &graph, &tape, rate);
            match committed { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); } }
            model.updates = model.updates + 1u64;
            return TensorResult::Ok(loss_number);
        },
        TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }
    }
}

fn training_tape(graph: &mut Graph, model: &Model, input: &Tensor, expected: &Tensor) -> TensorResult<TrainingTape>{ let result_input = crate::graph::input(&mut *graph, &*input); match result_input { TensorResult::Ok(var_input) => { let result_weights1 = crate::graph::input(&mut *graph, &*model.weights1); match result_weights1 { TensorResult::Ok(var_weights1) => { let result_bias1 = crate::graph::input(&mut *graph, &*model.bias1); match result_bias1 { TensorResult::Ok(var_bias1) => { let result_weights2 = crate::graph::input(&mut *graph, &*model.weights2); match result_weights2 { TensorResult::Ok(var_weights2) => { let result_bias2 = crate::graph::input(&mut *graph, &*model.bias2); match result_bias2 { TensorResult::Ok(var_bias2) => { let result_expected = crate::graph::input(&mut *graph, &*expected); match result_expected { TensorResult::Ok(var_expected) => { let result_z1 = crate::graph::matmul(&mut *graph, &var_input, &var_weights1); match result_z1 { TensorResult::Ok(var_z1) => { let result_h1 = crate::graph::add_rows(&mut *graph, &var_z1, &var_bias1); match result_h1 { TensorResult::Ok(var_h1) => { let result_hidden = crate::graph::relu(&mut *graph, &var_h1); match result_hidden { TensorResult::Ok(var_hidden) => { let result_z2 = crate::graph::matmul(&mut *graph, &var_hidden, &var_weights2); match result_z2 { TensorResult::Ok(var_z2) => { let result_prediction = crate::graph::add_rows(&mut *graph, &var_z2, &var_bias2); match result_prediction { TensorResult::Ok(var_prediction) => { let result_loss = crate::graph::mse(&mut *graph, &var_prediction, &var_expected); match result_loss { TensorResult::Ok(var_loss) => { return TensorResult::Ok(TrainingTape { weights1: var_weights1, bias1: var_bias1, weights2: var_weights2, bias2: var_bias2, loss: var_loss }); }, TensorResult::Err(error_loss) => { return TensorResult::<TrainingTape>:: Err(error_loss); } } }, TensorResult::Err(error_prediction) => { return TensorResult::<TrainingTape>:: Err(error_prediction); } } }, TensorResult::Err(error_z2) => { return TensorResult::<TrainingTape>:: Err(error_z2); } } }, TensorResult::Err(error_hidden) => { return TensorResult::<TrainingTape>:: Err(error_hidden); } } }, TensorResult::Err(error_h1) => { return TensorResult::<TrainingTape>:: Err(error_h1); } } }, TensorResult::Err(error_z1) => { return TensorResult::<TrainingTape>:: Err(error_z1); } } }, TensorResult::Err(error_expected) => { return TensorResult::<TrainingTape>:: Err(error_expected); } } }, TensorResult::Err(error_bias2) => { return TensorResult::<TrainingTape>:: Err(error_bias2); } } }, TensorResult::Err(error_weights2) => { return TensorResult::<TrainingTape>:: Err(error_weights2); } } }, TensorResult::Err(error_bias1) => { return TensorResult::<TrainingTape>:: Err(error_bias1); } } }, TensorResult::Err(error_weights1) => { return TensorResult::<TrainingTape>:: Err(error_weights1); } } }, TensorResult::Err(error_input) => { return TensorResult::<TrainingTape>:: Err(error_input); } } }
