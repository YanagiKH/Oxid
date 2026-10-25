mod codec;
pub mod graph;
pub mod model;
pub mod optimizer;

use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;

pub enum Error {
    NegativeShape,
    SizeOverflow,
    ShapeMismatch,
    Bounds,
    AllocationFailed,
    MemoryLimit,
    InvalidStorage,
    EmptyReduction,
    InvalidNode,
    GraphCapacity,
    NonFinite,
    InvalidRate,
    InvalidOptimizer,
    StepOverflow,
    InvalidCheckpoint,
    IoError(i32),
    WriteZero
}

pub enum TensorResult<T> { Ok(T), Err(Error) }

pub struct Tensor { rows: i32, columns: i32, data: ByteBuf }

pub fn empty() -> Tensor { return Tensor { rows: 0, columns: 0, data: new() }; }
pub fn row_count(value: &Tensor) -> i32 { return value.rows; }
pub fn column_count(value: &Tensor) -> i32 { return value.columns; }

pub fn from_slice(rows: i32, columns: i32, values: &[f64]) -> TensorResult<Tensor>{
    if rows < 0 || columns < 0 { return TensorResult::<Tensor>:: Err(Error::NegativeShape); }
    if rows != 0 {
        if columns > 268435455 / rows { return TensorResult::<Tensor>:: Err(Error::SizeOverflow); }
    }
    if values.len() != rows * columns { return TensorResult::<Tensor>:: Err(Error::ShapeMismatch); }
    let mut output = empty();
    let made = filled(rows, columns, 0.0);
    match made {
        TensorResult::Ok(value) => { output = value; },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
    copy_from(&*values, &mut output.data);
    return TensorResult::Ok(output);
}

fn copy_from(values: &[f64], bytes: &mut [u8]) -> () {
    for element in 0..values.len() { crate::codec::store(&mut *bytes, element, values[element]); }
    return;
}

pub fn copy_values(value: &Tensor, target: &mut [f64]) -> TensorResult<()>{
    if !valid(&*value) { return TensorResult::<()>:: Err(Error::InvalidStorage); }
    if target.len() != value.rows * value.columns { return TensorResult::<()>:: Err(Error::ShapeMismatch); }
    copy_to(&*value.data, &mut *target);
    return TensorResult::Ok(());
}

fn copy_to(bytes: &[u8], values: &mut [f64]) -> () {
    for element in 0..values.len() { values[element] = crate::codec::load(&*bytes, element); }
    return;
}

pub fn duplicate(value: &Tensor) -> TensorResult<Tensor>{
    if !valid(&*value) { return TensorResult::<Tensor>:: Err(Error::InvalidStorage); }
    let mut output = empty();
    let made = filled(value.rows, value.columns, 0.0);
    match made {
        TensorResult::Ok(tensor) => { output = tensor; },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
    copy_bytes(&*value.data, &mut output.data);
    return TensorResult::Ok(output);
}

fn copy_bytes(source: &[u8], target: &mut [u8]) -> () {
    for byte in 0..source.len() { target[byte] = source[byte]; }
    return;
}

pub fn matmul(left: &Tensor, right: &Tensor) -> TensorResult<Tensor>{
    if !valid(&*left) || !valid(&*right) { return TensorResult::<Tensor>:: Err(Error::InvalidStorage); }
    if left.columns != right.rows { return TensorResult::<Tensor>:: Err(Error::ShapeMismatch); }
    let mut output = empty();
    let made = filled(left.rows, right.columns, 0.0);
    match made {
        TensorResult::Ok(tensor) => { output = tensor; },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
    let rows = left.rows;
    let inner = left.columns;
    let columns = right.columns;
    matmul_into(&*left.data, &*right.data, &mut output.data, rows, inner, columns);
    return TensorResult::Ok(output);
}

fn matmul_into(left: &[u8], right: &[u8], output: &mut [u8], rows: i32, inner: i32, columns: i32) -> () {
    for row in 0..rows {
        for column in 0..columns {
            let mut sum = 0.0;
            for index in 0..inner {
                let a = crate::codec::load(&*left, row * inner + index);
                let b = crate::codec::load(&*right, index * columns + column);
                sum = sum + a * b;
            }
            crate::codec::store(&mut *output, row * columns + column, sum);
        }
    }
    return;
}

pub fn add(left: &Tensor, right: &Tensor) -> TensorResult<Tensor>{
    return pairwise(&*left, &*right, 0);
}

pub fn multiply(left: &Tensor, right: &Tensor) -> TensorResult<Tensor>{
    return pairwise(&*left, &*right, 1);
}

fn pairwise(left: &Tensor, right: &Tensor, operation: i32) -> TensorResult<Tensor>{
    if !valid(&*left) || !valid(&*right) { return TensorResult::<Tensor>:: Err(Error::InvalidStorage); }
    if left.rows != right.rows || left.columns != right.columns { return TensorResult::<Tensor>:: Err(Error::ShapeMismatch); }
    let mut output = empty();
    let made = filled(left.rows, left.columns, 0.0);
    match made {
        TensorResult::Ok(tensor) => { output = tensor; },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
    let elements = left.rows * left.columns;
    pairwise_into(&*left.data, &*right.data, &mut output.data, elements, operation);
    return TensorResult::Ok(output);
}

fn pairwise_into(left: &[u8], right: &[u8], output: &mut [u8], elements: i32, operation: i32) -> () {
    for element in 0..elements {
        let a = crate::codec::load(&*left, element);
        let b = crate::codec::load(&*right, element);
        let mut result = a + b;
        if operation == 1 { result = a * b; }
        crate::codec::store(&mut *output, element, result);
    }
    return;
}

pub fn relu(value: &Tensor) -> TensorResult<Tensor>{
    let mut output = empty();
    let made = duplicate(&*value);
    match made {
        TensorResult::Ok(tensor) => { output = tensor; },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
    let elements = output.rows * output.columns;
    relu_into(&mut output.data, elements);
    return TensorResult::Ok(output);
}

fn relu_into(bytes: &mut [u8], elements: i32) -> () {
    for element in 0..elements {
        let value = crate::codec::load(&*bytes, element);
        if value < 0.0 { crate::codec::store(&mut *bytes, element, 0.0); }
    }
    return;
}

pub fn mean(value: &Tensor) -> TensorResult<f64>{
    if !valid(&*value) { return TensorResult::<f64>:: Err(Error::InvalidStorage); }
    let elements = value.rows * value.columns;
    if elements == 0 { return TensorResult::<f64>:: Err(Error::EmptyReduction); }
    return TensorResult::Ok(mean_bytes(&*value.data, elements));
}

fn mean_bytes(bytes: &[u8], elements: i32) -> f64 {
    let mut sum = 0.0;
    for element in 0..elements { sum = sum + crate::codec::load(&*bytes, element); }
    return sum / elements.to_f64();
}

pub fn filled(rows: i32, columns: i32, value: f64) -> TensorResult<Tensor>{
    if rows < 0 || columns < 0 { return TensorResult::<Tensor>:: Err(Error::NegativeShape); }
    if rows != 0 {
        if columns > 268435455 / rows { return TensorResult::<Tensor>:: Err(Error::SizeOverflow); }
    }
    let elements = rows * columns;
    let mut data = new();
    let grown = try_resize(&mut data, elements * 8, 0u8);
    match grown {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return TensorResult::<Tensor>:: Err(Error::SizeOverflow); },
        GrowStatus::AllocationFailed => { return TensorResult::<Tensor>:: Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return TensorResult::<Tensor>:: Err(Error::MemoryLimit); }
    }
    if value.to_bits() != 0u64 { fill_values(&mut data, elements, value); }
    return TensorResult::Ok(Tensor { rows: rows, columns: columns, data: data });
}

fn fill_values(bytes: &mut [u8], elements: i32, value: f64) -> () {
    for element in 0..elements { crate::codec::store(&mut *bytes, element, value); }
    return;
}

pub fn valid(value: &Tensor) -> bool {
    if value.rows < 0 || value.columns < 0 { return false; }
    if value.rows != 0 {
        if value.columns > 268435455 / value.rows { return false; }
    }
    return len(&*value.data) == value.rows * value.columns * 8;
}

pub fn get(value: &Tensor, row: i32, column: i32) -> TensorResult<f64>{
    if !valid(&*value) { return TensorResult::<f64>:: Err(Error::InvalidStorage); }
    if row < 0 || row >= value.rows || column < 0 || column >= value.columns {
        return TensorResult::<f64>:: Err(Error::Bounds);
    }
    return TensorResult::Ok(crate::codec::load(&*value.data, row * value.columns + column));
}

pub fn set(value: &mut Tensor, row: i32, column: i32, number: f64) -> TensorResult<()>{
    if !valid(&*value) { return TensorResult::<()>:: Err(Error::InvalidStorage); }
    if row < 0 || row >= value.rows || column < 0 || column >= value.columns {
        return TensorResult::<()>:: Err(Error::Bounds);
    }
    let element = row * value.columns + column;
    crate::codec::store(&mut *value.data, element, number);
    return TensorResult::Ok(());
}

pub fn add_rows(left: &Tensor, bias: &Tensor) -> TensorResult<Tensor>{
    if !valid(&*left) || !valid(&*bias) { return TensorResult::<Tensor>:: Err(Error::InvalidStorage); }
    if bias.rows != 1 || left.columns != bias.columns { return TensorResult::<Tensor>:: Err(Error::ShapeMismatch); }
    let made = duplicate(&*left);
    match made {
        TensorResult::Ok(tensor) => {
            let mut output = tensor;
            let elements = left.rows * left.columns;
            let columns = left.columns;
            add_rows_into(&mut output.data, &*bias.data, elements, columns);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
}

fn add_rows_into(target: &mut [u8], bias: &[u8], elements: i32, columns: i32) -> () {
    for element in 0..elements {
        let previous = crate::codec::load(&*target, element);
        let offset = crate::codec::load(&*bias, element % columns);
        crate::codec::store(&mut *target, element, previous + offset);
    }
    return;
}

pub fn mean_squared_error(left: &Tensor, right: &Tensor) -> TensorResult<f64>{
    if !valid(&*left) || !valid(&*right) { return TensorResult::<f64>:: Err(Error::InvalidStorage); }
    if left.rows != right.rows || left.columns != right.columns { return TensorResult::<f64>:: Err(Error::ShapeMismatch); }
    let elements = left.rows * left.columns;
    if elements == 0 { return TensorResult::<f64>:: Err(Error::EmptyReduction); }
    return TensorResult::Ok(mse_bytes(&*left.data, &*right.data, elements));
}

fn mse_bytes(left: &[u8], right: &[u8], elements: i32) -> f64 {
    let mut sum = 0.0;
    for element in 0..elements {
        let delta = crate::codec::load(&*left, element) - crate::codec::load(&*right, element);
        sum = sum + delta * delta;
    }
    return sum / elements.to_f64();
}
