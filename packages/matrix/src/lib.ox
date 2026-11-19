use alloc::bytes::GrowStatus;
use crate::vector::Vector;
use crate::vector::Read as VectorRead;
use crate::vector::new as new_vector;
use crate::vector::len as vector_len;
use crate::vector::try_reserve;
use crate::vector::try_push;
use crate::vector::get as vector_get;
use crate::vector::set as vector_set;
use crate::vector::take as take_vector;

pub struct Matrix {
    row_count: i32,
    column_count: i32,
    data: Vector
}

pub enum Read {
    Value(f64),
    OutOfBounds
}

pub enum Error {
    InvalidShape,
    ShapeMismatch,
    CapacityOverflow,
    AllocationFailed,
    MemoryLimit
}

pub enum MatrixResult {
    Ok(Matrix),
    Err(Error)
}

pub enum FromVector {
    Ok(Matrix),
    InvalidShape(Vector)
}

fn valid_shape(row_count: i32, column_count: i32) -> bool {
    if row_count < 0 || column_count < 0 { return false; }
    if row_count > 0 && column_count > 268435455 / row_count { return false; }
    return true;
}

pub fn rows(matrix: &Matrix) -> i32 {
    return matrix.row_count;
}

pub fn columns(matrix: &Matrix) -> i32 {
    return matrix.column_count;
}

pub fn zeros(row_count: i32, column_count: i32) -> MatrixResult {
    if row_count < 0 || column_count < 0 { return MatrixResult::Err(Error::InvalidShape); }
    if !valid_shape(row_count, column_count) { return MatrixResult::Err(Error::CapacityOverflow); }
    let element_count = row_count * column_count;
    let mut elements = new_vector();
    let reserved = try_reserve(&mut elements, element_count);
    match reserved {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return MatrixResult::Err(Error::CapacityOverflow); },
        GrowStatus::AllocationFailed => { return MatrixResult::Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return MatrixResult::Err(Error::MemoryLimit); },
    }
    let mut index = 0;
    while index < element_count {
        let appended = try_push(&mut elements, 0.0);
        match appended {
            GrowStatus::Ok => {},
            GrowStatus::CapacityOverflow => { return MatrixResult::Err(Error::CapacityOverflow); },
            GrowStatus::AllocationFailed => { return MatrixResult::Err(Error::AllocationFailed); },
            GrowStatus::MemoryLimit => { return MatrixResult::Err(Error::MemoryLimit); },
        }
        index = index + 1;
    }
    return MatrixResult::Ok(Matrix { row_count: row_count, column_count: column_count, data: elements });
}

pub fn from_vector(input: Vector, row_count: i32, column_count: i32) -> FromVector {
    if !valid_shape(row_count, column_count) { return FromVector::InvalidShape(input); }
    if vector_len(&input) != row_count * column_count { return FromVector::InvalidShape(input); }
    return FromVector::Ok(Matrix { row_count: row_count, column_count: column_count, data: input });
}

pub fn from_slice(row_count: i32, column_count: i32, values: &[f64]) -> MatrixResult {
    if row_count < 0 || column_count < 0 { return MatrixResult::Err(Error::InvalidShape); }
    if !valid_shape(row_count, column_count) { return MatrixResult::Err(Error::CapacityOverflow); }
    let element_count = row_count * column_count;
    if values.len() != element_count { return MatrixResult::Err(Error::InvalidShape); }
    let mut elements = new_vector();
    let reserved = try_reserve(&mut elements, element_count);
    match reserved {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return MatrixResult::Err(Error::CapacityOverflow); },
        GrowStatus::AllocationFailed => { return MatrixResult::Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return MatrixResult::Err(Error::MemoryLimit); },
    }
    let mut index = 0;
    while index < element_count {
        let appended = try_push(&mut elements, values[index]);
        match appended {
            GrowStatus::Ok => {},
            GrowStatus::CapacityOverflow => { return MatrixResult::Err(Error::CapacityOverflow); },
            GrowStatus::AllocationFailed => { return MatrixResult::Err(Error::AllocationFailed); },
            GrowStatus::MemoryLimit => { return MatrixResult::Err(Error::MemoryLimit); },
        }
        index = index + 1;
    }
    return MatrixResult::Ok(Matrix { row_count: row_count, column_count: column_count, data: elements });
}

pub fn into_vector(input: Matrix) -> Vector {
    let mut matrix = input;
    return take_vector(&mut matrix.data);
}

pub fn get(matrix: &Matrix, row: i32, column: i32) -> Read {
    if row < 0 || row >= matrix.row_count || column < 0 || column >= matrix.column_count {
        return Read::OutOfBounds;
    }
    let found = vector_get(&*matrix.data, row * matrix.column_count + column);
    match found {
        VectorRead::Value(value) => { return Read::Value(value); },
        VectorRead::OutOfBounds => { return Read::OutOfBounds; },
    }
}

pub fn set(matrix: &mut Matrix, row: i32, column: i32, value: f64) -> bool {
    if row < 0 || row >= matrix.row_count || column < 0 || column >= matrix.column_count {
        return false;
    }
    let offset = row * matrix.column_count + column;
    return vector_set(&mut *matrix.data, offset, value);
}

pub fn transpose(input: &Matrix) -> MatrixResult {
    let created = zeros(input.column_count, input.row_count);
    match created {
        MatrixResult::Ok(matrix) => {
            let mut output = matrix;
            if vector_len(&*input.data) == 0 { return MatrixResult::Ok(output); }
            let mut row = 0;
            while row < input.row_count {
                let mut column = 0;
                while column < input.column_count {
                    let found = get(&*input, row, column);
                    match found {
                        Read::Value(value) => {
                            if !set(&mut output, column, row, value) { return MatrixResult::Err(Error::InvalidShape); }
                        },
                        Read::OutOfBounds => { return MatrixResult::Err(Error::InvalidShape); },
                    }
                    column = column + 1;
                }
                row = row + 1;
            }
            return MatrixResult::Ok(output);
        },
        MatrixResult::Err(error) => { return MatrixResult::Err(error); },
    }
}

fn dot(left: &Matrix, right: &Matrix, row: i32, column: i32) -> Read {
    let mut total = 0.0;
    let mut inner = 0;
    while inner < left.column_count {
        let first = get(&*left, row, inner);
        let second = get(&*right, inner, column);
        match first {
            Read::Value(first_value) => {
                match second {
                    Read::Value(second_value) => { total = total + first_value * second_value; },
                    Read::OutOfBounds => { return Read::OutOfBounds; },
                }
            },
            Read::OutOfBounds => { return Read::OutOfBounds; },
        }
        inner = inner + 1;
    }
    return Read::Value(total);
}

pub fn matmul(left: &Matrix, right: &Matrix) -> MatrixResult {
    if left.column_count != right.row_count { return MatrixResult::Err(Error::ShapeMismatch); }
    let created = zeros(left.row_count, right.column_count);
    match created {
        MatrixResult::Ok(matrix) => {
            let mut output = matrix;
            if vector_len(&output.data) == 0 { return MatrixResult::Ok(output); }
            let mut row = 0;
            while row < left.row_count {
                let mut column = 0;
                while column < right.column_count {
                    let computed = dot(&*left, &*right, row, column);
                    match computed {
                        Read::Value(value) => {
                            if !set(&mut output, row, column, value) { return MatrixResult::Err(Error::InvalidShape); }
                        },
                        Read::OutOfBounds => { return MatrixResult::Err(Error::InvalidShape); },
                    }
                    column = column + 1;
                }
                row = row + 1;
            }
            return MatrixResult::Ok(output);
        },
        MatrixResult::Err(error) => { return MatrixResult::Err(error); },
    }
}
