use crate::Tensor;
use crate::TensorResult;
use crate::Error;
pub mod checkpoint;

pub struct Config {
    kind: u8,
    rate: f64,
    beta1: f64,
    beta2: f64,
    epsilon: f64,
    decay: f64,
    max_norm: f64,
    schedule_factor: f64,
    schedule_every: u64
}

pub struct State {
    rows: i32,
    columns: i32,
    config: Config,
    first: Tensor,
    second: Tensor,
    power1: f64,
    power2: f64,
    steps: u64
}

struct Clip { active: bool, scale: f64, root: f64, limit: f64 }
struct Update { parameter: f64, first: f64, second: f64 }

fn finite(value: f64) -> bool { return ((value.to_bits() >> 52) & 2047u64) != 2047u64; }

fn valid_config(config: &Config) -> bool {
    if config.kind > 2u8 || !finite(config.rate) || config.rate < 0.0 { return false; }
    if !finite(config.decay) || config.decay < 0.0 || !finite(config.max_norm) || config.max_norm < 0.0 { return false; }
    if !finite(config.schedule_factor) || config.schedule_factor < 0.0 || config.schedule_factor > 1.0 { return false; }
    if config.schedule_every == 0u64 && config.schedule_factor != 0.0 { return false; }
    if config.kind == 0u8 { return config.beta1 == 0.0 && config.beta2 == 0.0 && config.epsilon == 1.0 && config.decay == 0.0; }
    if config.kind == 1u8 && config.decay != 0.0 { return false; }
    return finite(config.beta1) && config.beta1 >= 0.0 && config.beta1 < 1.0 && finite(config.beta2) && config.beta2 >= 0.0 && config.beta2 < 1.0 && finite(config.epsilon) && config.epsilon > 0.0;
}

pub fn sgd(rate: f64) -> TensorResult<Config>{
    if !finite(rate) || rate < 0.0 { return TensorResult::<Config>:: Err(Error::InvalidRate); }
    return TensorResult::Ok(Config { kind: 0u8, rate: rate, beta1: 0.0, beta2: 0.0, epsilon: 1.0, decay: 0.0, max_norm: 0.0, schedule_factor: 0.0, schedule_every: 0u64 });
}

pub fn adam(rate: f64, beta1: f64, beta2: f64, epsilon: f64) -> TensorResult<Config>{
    return configured(1u8, rate, beta1, beta2, epsilon, 0.0);
}

pub fn adamw(rate: f64, beta1: f64, beta2: f64, epsilon: f64, decay: f64) -> TensorResult<Config>{
    return configured(2u8, rate, beta1, beta2, epsilon, decay);
}

fn configured(kind: u8, rate: f64, beta1: f64, beta2: f64, epsilon: f64, decay: f64) -> TensorResult<Config>{
    let config = Config { kind: kind, rate: rate, beta1: beta1, beta2: beta2, epsilon: epsilon, decay: decay, max_norm: 0.0, schedule_factor: 0.0, schedule_every: 0u64 };
    if !valid_config(&config) { return TensorResult::<Config>:: Err(Error::InvalidOptimizer); }
    return TensorResult::Ok(config);
}

pub fn set_clip_norm(config: &mut Config, maximum: f64) -> TensorResult<()>{
    if !finite(maximum) || maximum < 0.0 { return TensorResult::<()>:: Err(Error::InvalidOptimizer); }
    config.max_norm = maximum;
    return TensorResult::Ok(());
}

pub fn set_step_decay(config: &mut Config, factor: f64, every: u64) -> TensorResult<()>{
    if !finite(factor) || factor < 0.0 || factor > 1.0 || every == 0u64 { return TensorResult::<()>:: Err(Error::InvalidOptimizer); }
    config.schedule_factor = factor;
    config.schedule_every = every;
    return TensorResult::Ok(());
}

pub fn initialize(rows: i32, columns: i32, config: Config) -> TensorResult<State>{
    if rows < 0 || columns < 0 { return TensorResult::<State>:: Err(Error::NegativeShape); }
    if rows != 0 { if columns > 268435455 / rows { return TensorResult::<State>:: Err(Error::SizeOverflow); } }
    if !valid_config(&config) { return TensorResult::<State>:: Err(Error::InvalidOptimizer); }
    let mut first = crate::empty();
    let mut second = crate::empty();
    if config.kind != 0u8 {
        let allocated = crate::filled(rows, columns, 0.0);
        match allocated { TensorResult::Ok(value) => { first = value; }, TensorResult::Err(error) => { return TensorResult::<State>:: Err(error); } }
        let allocated_second = crate::filled(rows, columns, 0.0);
        match allocated_second { TensorResult::Ok(value) => { second = value; }, TensorResult::Err(error) => { return TensorResult::<State>:: Err(error); } }
    }
    return TensorResult::Ok(State { rows: rows, columns: columns, config: config, first: first, second: second, power1: 1.0, power2: 1.0, steps: 0u64 });
}

pub fn initialize_sgd(rows: i32, columns: i32, rate: f64, steps: u64) -> TensorResult<State>{
    let selected_config = sgd(rate);
    match selected_config {
        TensorResult::Ok(config) => {
            let initialized = initialize(rows, columns, config);
            match initialized {
                TensorResult::Ok(value) => { let mut state = value; state.steps = steps; return TensorResult::Ok(state); },
                TensorResult::Err(error) => { return TensorResult::<State>:: Err(error); }
            }
        },
        TensorResult::Err(error) => { return TensorResult::<State>:: Err(error); }
    }
}

pub fn step_count(state: &State) -> u64 { return state.steps; }
pub fn row_count(state: &State) -> i32 { return state.rows; }
pub fn column_count(state: &State) -> i32 { return state.columns; }
pub fn base_learning_rate(state: &State) -> f64 { return state.config.rate; }
pub fn learning_rate(state: &State) -> f64 {
    if state.config.schedule_every == 0u64 { return state.config.rate; }
    return rate_power(state.config.rate, state.config.schedule_factor, state.steps / state.config.schedule_every);
}
pub fn first_moment(state: &State) -> TensorResult<Tensor>{
    if state.config.kind == 0u8 { return TensorResult::<Tensor>:: Err(Error::InvalidOptimizer); }
    return crate::duplicate(&*state.first);
}
pub fn second_moment(state: &State) -> TensorResult<Tensor>{
    if state.config.kind == 0u8 { return TensorResult::<Tensor>:: Err(Error::InvalidOptimizer); }
    return crate::duplicate(&*state.second);
}

pub fn set_base_learning_rate(state: &mut State, rate: f64) -> TensorResult<()>{
    if !finite(rate) || rate < 0.0 { return TensorResult::<()>:: Err(Error::InvalidRate); }
    state.config.rate = rate;
    return TensorResult::Ok(());
}

fn read(value: &Tensor, index: i32) -> f64 { return crate::codec::load(&*value.data, index); }
fn write(value: &mut Tensor, index: i32, number: f64) -> () { crate::codec::store(&mut *value.data, index, number); return; }

fn clipping(gradient: &Tensor, maximum: f64) -> TensorResult<Clip>{
    let mut scale = 0.0;
    let mut squares = 1.0;
    for index in 0..gradient.rows * gradient.columns {
        let value = read(&*gradient, index);
        if !finite(value) { return TensorResult::<Clip>:: Err(Error::NonFinite); }
        let mut magnitude = value;
        if magnitude < 0.0 { magnitude = -magnitude; }
        if magnitude != 0.0 && maximum > 0.0 {
            if scale < magnitude {
                let ratio = scale / magnitude;
                squares = 1.0 + squares * ratio * ratio;
                scale = magnitude;
            } else {
                let ratio = magnitude / scale;
                squares = squares + ratio * ratio;
            }
        }
    }
    let root = squares.sqrt();
    let mut active = false;
    if maximum > 0.0 && scale > 0.0 { active = maximum / scale < root; }
    return TensorResult::Ok(Clip { active: active, scale: scale, root: root, limit: maximum });
}

fn proposal(state: &State, index: i32, parameter: f64, derivative: f64, clip: &Clip, power1: f64, power2: f64, rate: f64) -> TensorResult<Update>{
    if !finite(parameter) || !finite(derivative) { return TensorResult::<Update>:: Err(Error::NonFinite); }
    let mut gradient = derivative;
    if clip.active { gradient = ((derivative / clip.scale) / clip.root) * clip.limit; }
    if state.config.kind == 0u8 {
        let candidate = parameter - rate * gradient;
        if !finite(candidate) { return TensorResult::<Update>:: Err(Error::NonFinite); }
        return TensorResult::Ok(Update { parameter: candidate, first: 0.0, second: 0.0 });
    }
    let first = state.config.beta1 * read(&*state.first, index) + (1.0 - state.config.beta1) * gradient;
    let square = gradient * gradient;
    let second = state.config.beta2 * read(&*state.second, index) + (1.0 - state.config.beta2) * square;
    let corrected_first = first / (1.0 - power1);
    let corrected_second = second / (1.0 - power2);
    let root = corrected_second.sqrt();
    let denominator = root + state.config.epsilon;
    let direction = corrected_first / denominator;
    let mut candidate = parameter - rate * direction;
    if state.config.decay != 0.0 { candidate = candidate - rate * state.config.decay * parameter; }
    if !finite(first) || !finite(second) || !finite(corrected_first) || !finite(corrected_second) || !finite(direction) || !finite(candidate) {
        return TensorResult::<Update>:: Err(Error::NonFinite);
    }
    return TensorResult::Ok(Update { parameter: candidate, first: first, second: second });
}

pub fn step(state: &mut State, parameter: &mut Tensor, gradient: &Tensor) -> TensorResult<()>{
    if state.steps == 18446744073709551615u64 { return TensorResult::<()>:: Err(Error::StepOverflow); }
    if !valid_config(&*state.config) { return TensorResult::<()>:: Err(Error::InvalidOptimizer); }
    if !crate::valid(&*parameter) || !crate::valid(&*gradient) { return TensorResult::<()>:: Err(Error::InvalidStorage); }
    if parameter.rows != state.rows || parameter.columns != state.columns || gradient.rows != state.rows || gradient.columns != state.columns {
        return TensorResult::<()>:: Err(Error::ShapeMismatch);
    }
    if state.config.kind != 0u8 {
        if !crate::valid(&*state.first) || !crate::valid(&*state.second) { return TensorResult::<()>:: Err(Error::InvalidStorage); }
        if state.first.rows != state.rows || state.first.columns != state.columns || state.second.rows != state.rows || state.second.columns != state.columns {
            return TensorResult::<()>:: Err(Error::ShapeMismatch);
        }
    }
    let mut clip = Clip { active: false, scale: 0.0, root: 1.0, limit: 0.0 };
    let clipped = clipping(&*gradient, state.config.max_norm);
    match clipped { TensorResult::Ok(value) => { clip = value; }, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let mut power1 = state.power1;
    let mut power2 = state.power2;
    if state.config.kind != 0u8 { power1 = power1 * state.config.beta1; power2 = power2 * state.config.beta2; }
    let rate = learning_rate(&*state);
    for index in 0..state.rows * state.columns {
        let checked = proposal(&*state, index, read(&*parameter, index), read(&*gradient, index), &clip, power1, power2, rate);
        match checked { TensorResult::Ok(value) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    }
    for index in 0..state.rows * state.columns {
        let proposed = proposal(&*state, index, read(&*parameter, index), read(&*gradient, index), &clip, power1, power2, rate);
        match proposed {
            TensorResult::Ok(value) => {
                write(&mut *parameter, index, value.parameter);
                if state.config.kind != 0u8 { write(&mut *state.first, index, value.first); write(&mut *state.second, index, value.second); }
            },
            TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); }
        }
    }
    state.power1 = power1;
    state.power2 = power2;
    state.steps = state.steps + 1u64;
    return TensorResult::Ok(());
}

pub fn decayed_rate(base: f64, factor: f64, every: u64, steps: u64) -> TensorResult<f64>{
    if !finite(base) || base < 0.0 || !finite(factor) || factor < 0.0 || factor > 1.0 || every == 0u64 {
        return TensorResult::<f64>:: Err(Error::InvalidOptimizer);
    }
    return TensorResult::Ok(rate_power(base, factor, steps / every));
}

fn rate_power(base: f64, factor: f64, periods: u64) -> f64 {
    let mut exponent = periods;
    let mut power = factor;
    let mut result = base;
    while exponent > 0u64 {
        let bit = exponent & 1u64;
        if bit != 0u64 { result = result * power; }
        exponent = exponent / 2u64;
        if exponent > 0u64 { power = power * power; }
    }
    return result;
}
