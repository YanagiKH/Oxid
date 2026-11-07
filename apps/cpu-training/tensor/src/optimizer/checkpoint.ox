use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use crate::TensorResult;
use crate::Error;
use crate::optimizer::State;
use crate::optimizer::Config;

fn powers_valid(kind: u8, steps: u64, beta1: f64, beta2: f64, first: f64, second: f64) -> bool {
    if !crate::optimizer::finite(first) || !crate::optimizer::finite(second) { return false; }
    if kind == 0u8 || steps == 0u64 { return first == 1.0 && second == 1.0; }
    return first >= 0.0 && first <= beta1 && second >= 0.0 && second <= beta2;
}

fn state_valid(state: &State) -> bool {
    if !crate::optimizer::valid_config(&*state.config) || state.rows < 0 || state.columns < 0 { return false; }
    if state.rows != 0 { if state.columns > 268435455 / state.rows { return false; } }
    if !powers_valid(state.config.kind, state.steps, state.config.beta1, state.config.beta2, state.power1, state.power2) { return false; }
    if !crate::valid(&*state.first) || !crate::valid(&*state.second) { return false; }
    if state.config.kind == 0u8 { return state.first.rows == 0 && state.first.columns == 0 && state.second.rows == 0 && state.second.columns == 0; }
    if state.first.rows != state.rows || state.first.columns != state.columns || state.second.rows != state.rows || state.second.columns != state.columns { return false; }
    for index in 0..state.rows * state.columns {
        let first = crate::optimizer::read(&*state.first, index);
        let second = crate::optimizer::read(&*state.second, index);
        if !crate::optimizer::finite(first) || !crate::optimizer::finite(second) || second < 0.0 { return false; }
        if state.steps == 0u64 && (first != 0.0 || second != 0.0) { return false; }
    }
    return true;
}

fn write_config(output: &mut [u8], config: &Config, power1: f64, power2: f64) -> () {
    crate::codec::store(&mut *output, 4, config.rate);
    crate::codec::store(&mut *output, 5, config.beta1);
    crate::codec::store(&mut *output, 6, config.beta2);
    crate::codec::store(&mut *output, 7, config.epsilon);
    crate::codec::store(&mut *output, 8, config.decay);
    crate::codec::store(&mut *output, 9, config.max_norm);
    crate::codec::store(&mut *output, 10, power1);
    crate::codec::store(&mut *output, 11, power2);
    crate::codec::store(&mut *output, 13, config.schedule_factor);
    crate::codec::put64(&mut *output, 120, config.schedule_every);
    return;
}

fn write_magic(output: &mut [u8]) -> () {
    let magic = b"OXOP0001";
    for index in 0..8 { output[index] = magic[index]; }
    return;
}

pub fn encode(state: &State) -> TensorResult<ByteBuf>{
    if !state_valid(&*state) { return TensorResult::<ByteBuf>:: Err(Error::InvalidOptimizer); }
    let elements = state.rows * state.columns;
    let mut payload = 0;
    if state.config.kind != 0u8 {
        if elements > 1048567 { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }
        payload = elements * 16;
    }
    let end = 128 + payload;
    let mut output = new();
    let grown = try_resize(&mut output, end + 4, 0u8);
    match grown { GrowStatus::Ok => {}, GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>:: Err(Error::SizeOverflow); }, GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>:: Err(Error::AllocationFailed); }, GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>:: Err(Error::MemoryLimit); } }
    write_magic(&mut output);
    let kind = state.config.kind;
    let rows = state.rows;
    let columns = state.columns;
    crate::codec::put32(&mut output, 8, kind.to_u32_checked());
    crate::codec::put32(&mut output, 12, rows.to_u32_checked());
    crate::codec::put32(&mut output, 16, columns.to_u32_checked());
    crate::codec::put64(&mut output, 24, state.steps);
    write_config(&mut output, &*state.config, state.power1, state.power2);
    crate::codec::put64(&mut output, 96, elements.to_u64_checked());
    crate::codec::put64(&mut output, 112, payload.to_u64_checked());
    if kind != 0u8 {
        for index in 0..elements {
            let first = crate::optimizer::read(&*state.first, index);
            let second = crate::optimizer::read(&*state.second, index);
            crate::codec::store(&mut output, 16 + index, first);
            crate::codec::store(&mut output, 16 + elements + index, second);
        }
    }
    let crc = crate::codec::checksum(&output, end);
    crate::codec::put32(&mut output, end, crc);
    return TensorResult::Ok(output);
}

fn read_config(bytes: &[u8], kind: u8) -> Config {
    return Config { kind: kind, rate: crate::codec::load(&*bytes, 4), beta1: crate::codec::load(&*bytes, 5), beta2: crate::codec::load(&*bytes, 6), epsilon: crate::codec::load(&*bytes, 7), decay: crate::codec::load(&*bytes, 8), max_norm: crate::codec::load(&*bytes, 9), schedule_factor: crate::codec::load(&*bytes, 13), schedule_every: crate::codec::get64(&*bytes, 120) };
}

pub fn decode(bytes: &[u8]) -> TensorResult<State>{
    if bytes.len() < 132 || bytes.len() > 16777216 { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
    let magic = b"OXOP0001";
    for index in 0..8 { if bytes[index] != magic[index] { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); } }
    let kind_bits = crate::codec::get32(&*bytes, 8);
    let row_bits = crate::codec::get32(&*bytes, 12);
    let column_bits = crate::codec::get32(&*bytes, 16);
    if kind_bits > 2u32 || row_bits > 2147483647u32 || column_bits > 2147483647u32 || crate::codec::get32(&*bytes, 20) != 0u32 {
        return TensorResult::<State>:: Err(Error::InvalidCheckpoint);
    }
    let kind = kind_bits.to_u8_checked();
    let rows = row_bits.to_i32_checked();
    let columns = column_bits.to_i32_checked();
    if rows != 0 { if columns > 268435455 / rows { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); } }
    let elements = rows * columns;
    if crate::codec::get64(&*bytes, 96) != elements.to_u64_checked() { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
    let mut payload = 0;
    if kind != 0u8 {
        if elements > 1048567 { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
        payload = elements * 16;
    }
    let end = 128 + payload;
    if bytes.len() != end + 4 || crate::codec::get64(&*bytes, 112) != payload.to_u64_checked() || crate::codec::get32(&*bytes, end) != crate::codec::checksum(&*bytes, end) { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
    let config = read_config(&*bytes, kind);
    let power1 = crate::codec::load(&*bytes, 10);
    let power2 = crate::codec::load(&*bytes, 11);
    let steps = crate::codec::get64(&*bytes, 24);
    if !crate::optimizer::valid_config(&config) || !powers_valid(kind, steps, config.beta1, config.beta2, power1, power2) { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
    if kind != 0u8 {
        for index in 0..elements {
            let first = crate::codec::load(&*bytes, 16 + index);
            let second = crate::codec::load(&*bytes, 16 + elements + index);
            if !crate::optimizer::finite(first) || !crate::optimizer::finite(second) || second < 0.0 { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
            if steps == 0u64 && (first != 0.0 || second != 0.0) { return TensorResult::<State>:: Err(Error::InvalidCheckpoint); }
        }
    }
    return restore(&*bytes, config);
}

fn restore(bytes: &[u8], config: Config) -> TensorResult<State>{
    let row_bits = crate::codec::get32(&*bytes, 12);
    let column_bits = crate::codec::get32(&*bytes, 16);
    let rows = row_bits.to_i32_checked();
    let columns = column_bits.to_i32_checked();
    let initialized = crate::optimizer::initialize(rows, columns, config);
    match initialized {
        TensorResult::Ok(value) => { return restored(value, &*bytes); },
        TensorResult::Err(error) => { return TensorResult::<State>:: Err(error); }
    }
}

fn restored(value: State, bytes: &[u8]) -> TensorResult<State>{
    let mut state = value;
    if state.config.kind != 0u8 {
        let elements = state.rows * state.columns;
        for index in 0..elements {
            let first = crate::codec::load(&*bytes, 16 + index);
            let second = crate::codec::load(&*bytes, 16 + elements + index);
            crate::optimizer::write(&mut state.first, index, first);
            crate::optimizer::write(&mut state.second, index, second);
        }
    }
    state.power1 = crate::codec::load(&*bytes, 10);
    state.power2 = crate::codec::load(&*bytes, 11);
    state.steps = crate::codec::get64(&*bytes, 24);
    return TensorResult::Ok(state);
}
