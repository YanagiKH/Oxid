use crate::Tensor;
use crate::TensorResult;
use crate::Error;
use crate::graph::Graph;
use crate::graph::Variable;
pub mod checkpoint;
pub mod session;

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
struct Gradients { values: Tensor, loss: f64 }

pub fn update_count(model: &Model) -> u64 { return model.updates; }
pub fn input_count(model: &Model) -> i32 { return model.weights1.rows; }
pub fn output_count(model: &Model) -> i32 { return model.weights2.columns; }

pub fn parameter_elements(model: &Model) -> i32 {
    if !crate::valid(&*model.weights1) || !crate::valid(&*model.bias1) || !crate::valid(&*model.weights2) || !crate::valid(&*model.bias2) { return -1; }
    let first = model.weights1.rows * model.weights1.columns;
    let second = model.bias1.rows * model.bias1.columns;
    let third = model.weights2.rows * model.weights2.columns;
    let fourth = model.bias2.rows * model.bias2.columns;
    if second > 268435455 - first { return -1; }
    let subtotal = first + second;
    if third > 268435455 - subtotal { return -1; }
    let almost = subtotal + third;
    if fourth > 268435455 - almost { return -1; }
    return almost + fourth;
}

fn copy_parameter(source: &Tensor, target: &mut Tensor, start: i32) -> () {
    for index in 0..source.rows * source.columns {
        let value = crate::codec::load(&*source.data, index);
        crate::codec::store(&mut *target.data, start + index, value);
    }
    return;
}

pub fn parameters(model: &Model) -> TensorResult<Tensor>{
    let elements = parameter_elements(&*model);
    if elements < 0 { return TensorResult::<Tensor>:: Err(Error::InvalidStorage); }
    let allocated = crate::filled(1, elements, 0.0);
    match allocated {
        TensorResult::Ok(value) => {
            let mut output = value;
            let first = model.weights1.rows * model.weights1.columns;
            let second = model.bias1.rows * model.bias1.columns;
            let third = model.weights2.rows * model.weights2.columns;
            copy_parameter(&*model.weights1, &mut output, 0);
            copy_parameter(&*model.bias1, &mut output, first);
            copy_parameter(&*model.weights2, &mut output, first + second);
            copy_parameter(&*model.bias2, &mut output, first + second + third);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
}

fn restore_parameter(source: &Tensor, target: &mut Tensor, start: i32) -> () {
    for index in 0..target.rows * target.columns {
        let value = crate::codec::load(&*source.data, start + index);
        crate::codec::store(&mut *target.data, index, value);
    }
    return;
}

fn commit_parameters(model: &mut Model, source: &Tensor) -> () {
    let first = model.weights1.rows * model.weights1.columns;
    let second = model.bias1.rows * model.bias1.columns;
    let third = model.weights2.rows * model.weights2.columns;
    restore_parameter(&*source, &mut *model.weights1, 0);
    restore_parameter(&*source, &mut *model.bias1, first);
    restore_parameter(&*source, &mut *model.weights2, first + second);
    restore_parameter(&*source, &mut *model.bias2, first + second + third);
    return;
}

pub fn set_parameters(model: &mut Model, source: &Tensor) -> TensorResult<()>{
    let elements = parameter_elements(&*model);
    if elements < 0 || !crate::valid(&*source) { return TensorResult::<()>:: Err(Error::InvalidStorage); }
    if source.rows != 1 || source.columns != elements { return TensorResult::<()>:: Err(Error::ShapeMismatch); }
    for index in 0..elements { if !finite(crate::codec::load(&*source.data, index)) { return TensorResult::<()>:: Err(Error::NonFinite); } }
    commit_parameters(&mut *model, &*source);
    return TensorResult::Ok(());
}

fn copy_gradient(graph: &Graph, variable: &Variable, rows: i32, columns: i32, output: &mut Tensor, start: i32) -> TensorResult<()>{
    for row in 0..rows {
        for column in 0..columns {
            let read = crate::graph::gradient_at(&*graph, &*variable, row, column);
            match read {
                TensorResult::Ok(value) => { crate::codec::store(&mut *output.data, start + row * columns + column, value); },
                TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); }
            }
        }
    }
    return TensorResult::Ok(());
}

fn packed_gradients(graph: &Graph, tape: &TrainingTape, model: &Model) -> TensorResult<Tensor>{
    let elements = parameter_elements(&*model);
    if elements < 0 { return TensorResult::<Tensor>:: Err(Error::InvalidStorage); }
    let allocated = crate::filled(1, elements, 0.0);
    match allocated {
        TensorResult::Ok(value) => {
            let mut output = value;
            let first = model.weights1.rows * model.weights1.columns;
            let second = model.bias1.rows * model.bias1.columns;
            let third = model.weights2.rows * model.weights2.columns;
            let copied1 = copy_gradient(&*graph, &*tape.weights1, model.weights1.rows, model.weights1.columns, &mut output, 0);
            match copied1 { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); } }
            let copied2 = copy_gradient(&*graph, &*tape.bias1, model.bias1.rows, model.bias1.columns, &mut output, first);
            match copied2 { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); } }
            let copied3 = copy_gradient(&*graph, &*tape.weights2, model.weights2.rows, model.weights2.columns, &mut output, first + second);
            match copied3 { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); } }
            let copied4 = copy_gradient(&*graph, &*tape.bias2, model.bias2.rows, model.bias2.columns, &mut output, first + second + third);
            match copied4 { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); } }
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
}

fn gradients(model: &Model, input: &Tensor, expected: &Tensor) -> TensorResult<Gradients>{
    let mut graph = crate::graph::create();
    let made = training_tape(&mut graph, &*model, &*input, &*expected);
    match made {
        TensorResult::Ok(tape) => {
            let backward = crate::graph::backward(&mut graph, &tape.loss);
            match backward { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Gradients>:: Err(error); } }
            let mut objective_value = 0.0;
            let read = crate::graph::value_at(&graph, &tape.loss, 0, 0);
            match read { TensorResult::Ok(value) => { objective_value = value; }, TensorResult::Err(error) => { return TensorResult::<Gradients>:: Err(error); } }
            if !finite(objective_value) { return TensorResult::<Gradients>:: Err(Error::NonFinite); }
            let packed = packed_gradients(&graph, &tape, &*model);
            match packed {
                TensorResult::Ok(values) => { return TensorResult::Ok(Gradients { values: values, loss: objective_value }); },
                TensorResult::Err(error) => { return TensorResult::<Gradients>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Gradients>:: Err(error); }
    }
}

pub fn train_with_optimizer(model: &mut Model, state: &mut crate::optimizer::State, input: &Tensor, expected: &Tensor) -> TensorResult<f64>{
    if model.updates == 18446744073709551615u64 { return TensorResult::<f64>:: Err(Error::StepOverflow); }
    let elements = parameter_elements(&*model);
    if elements < 0 { return TensorResult::<f64>:: Err(Error::InvalidStorage); }
    if crate::optimizer::row_count(&*state) != 1 || crate::optimizer::column_count(&*state) != elements { return TensorResult::<f64>:: Err(Error::ShapeMismatch); }
    let computed = gradients(&*model, &*input, &*expected);
    match computed {
        TensorResult::Ok(gradient) => {
            let copied = parameters(&*model);
            match copied {
                TensorResult::Ok(value) => {
                    let mut target = value;
                    let applied = crate::optimizer::step(&mut *state, &mut target, &gradient.values);
                    match applied { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); } }
                    commit_parameters(&mut *model, &target);
                    model.updates = model.updates + 1u64;
                    return TensorResult::Ok(gradient.loss);
                },
                TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }
    }
}

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
    return number.sqrt();
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

pub fn train_step(model: &mut Model, input: &Tensor, expected: &Tensor, rate: f64) -> TensorResult<f64>{
    if model.updates == 18446744073709551615u64 { return TensorResult::<f64>:: Err(Error::StepOverflow); }
    if !finite(rate) || rate < 0.0 { return TensorResult::<f64>:: Err(Error::InvalidRate); }
    let initialized = crate::optimizer::initialize_sgd(1, parameter_elements(&*model), rate, model.updates);
    match initialized {
        TensorResult::Ok(value) => { let mut state = value; return train_with_optimizer(&mut *model, &mut state, &*input, &*expected); },
        TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }
    }
}

fn training_tape(graph: &mut Graph, model: &Model, input: &Tensor, expected: &Tensor) -> TensorResult<TrainingTape>{ let result_input = crate::graph::input(&mut *graph, &*input); match result_input { TensorResult::Ok(var_input) => { let result_weights1 = crate::graph::input(&mut *graph, &*model.weights1); match result_weights1 { TensorResult::Ok(var_weights1) => { let result_bias1 = crate::graph::input(&mut *graph, &*model.bias1); match result_bias1 { TensorResult::Ok(var_bias1) => { let result_weights2 = crate::graph::input(&mut *graph, &*model.weights2); match result_weights2 { TensorResult::Ok(var_weights2) => { let result_bias2 = crate::graph::input(&mut *graph, &*model.bias2); match result_bias2 { TensorResult::Ok(var_bias2) => { let result_expected = crate::graph::input(&mut *graph, &*expected); match result_expected { TensorResult::Ok(var_expected) => { let result_z1 = crate::graph::matmul(&mut *graph, &var_input, &var_weights1); match result_z1 { TensorResult::Ok(var_z1) => { let result_h1 = crate::graph::add_rows(&mut *graph, &var_z1, &var_bias1); match result_h1 { TensorResult::Ok(var_h1) => { let result_hidden = crate::graph::relu(&mut *graph, &var_h1); match result_hidden { TensorResult::Ok(var_hidden) => { let result_z2 = crate::graph::matmul(&mut *graph, &var_hidden, &var_weights2); match result_z2 { TensorResult::Ok(var_z2) => { let result_prediction = crate::graph::add_rows(&mut *graph, &var_z2, &var_bias2); match result_prediction { TensorResult::Ok(var_prediction) => { let result_loss = crate::graph::mse(&mut *graph, &var_prediction, &var_expected); match result_loss { TensorResult::Ok(var_loss) => { return TensorResult::Ok(TrainingTape { weights1: var_weights1, bias1: var_bias1, weights2: var_weights2, bias2: var_bias2, loss: var_loss }); }, TensorResult::Err(error_loss) => { return TensorResult::<TrainingTape>:: Err(error_loss); } } }, TensorResult::Err(error_prediction) => { return TensorResult::<TrainingTape>:: Err(error_prediction); } } }, TensorResult::Err(error_z2) => { return TensorResult::<TrainingTape>:: Err(error_z2); } } }, TensorResult::Err(error_hidden) => { return TensorResult::<TrainingTape>:: Err(error_hidden); } } }, TensorResult::Err(error_h1) => { return TensorResult::<TrainingTape>:: Err(error_h1); } } }, TensorResult::Err(error_z1) => { return TensorResult::<TrainingTape>:: Err(error_z1); } } }, TensorResult::Err(error_expected) => { return TensorResult::<TrainingTape>:: Err(error_expected); } } }, TensorResult::Err(error_bias2) => { return TensorResult::<TrainingTape>:: Err(error_bias2); } } }, TensorResult::Err(error_weights2) => { return TensorResult::<TrainingTape>:: Err(error_weights2); } } }, TensorResult::Err(error_bias1) => { return TensorResult::<TrainingTape>:: Err(error_bias1); } } }, TensorResult::Err(error_weights1) => { return TensorResult::<TrainingTape>:: Err(error_weights1); } } }, TensorResult::Err(error_input) => { return TensorResult::<TrainingTape>:: Err(error_input); } } }
