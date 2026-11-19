use alloc::bytes::GrowStatus;
use crate::vector::Read as VectorRead;
use crate::matrix::Read;
use crate::vector::new;
use crate::vector::len as vector_len;
use crate::vector::try_push;
use crate::vector::get as vector_get;
use crate::matrix::Matrix;
use crate::matrix::MatrixResult;
use crate::matrix::Error;
use crate::matrix::FromVector;
use crate::matrix::zeros;
use crate::matrix::from_slice;
use crate::matrix::from_vector;
use crate::matrix::into_vector;
use crate::matrix::rows;
use crate::matrix::columns;
use crate::matrix::get;
use crate::matrix::set;
use crate::matrix::transpose;
use crate::matrix::matmul;

fn error_code(result: MatrixResult) -> i32 {
    match result {
        MatrixResult::Ok(matrix) => { return 0; },
        MatrixResult::Err(error) => {
            match error {
                Error::InvalidShape => { return 1; },
                Error::ShapeMismatch => { return 2; },
                Error::CapacityOverflow => { return 3; },
                Error::AllocationFailed => { return 4; },
                Error::MemoryLimit => { return 5; },
            }
        },
    }
}

fn same(matrix: &Matrix, row_count: i32, column_count: i32, expected: &[f64]) -> bool {
    if rows(&*matrix) != row_count || columns(&*matrix) != column_count { return false; }
    if expected.len() != row_count * column_count { return false; }
    let mut index = 0;
    while index < expected.len() {
        let found = get(&*matrix, index / column_count, index % column_count);
        let expected_value = expected[index];
        match found {
            Read::Value(value) => { if value.to_bits() != expected_value.to_bits() { return false; } },
            Read::OutOfBounds => { return false; },
        }
        index = index + 1;
    }
    return true;
}

fn missing(matrix: &Matrix, row: i32, column: i32) -> bool {
    let found = get(&*matrix, row, column);
    match found {
        Read::Value(value) => { return false; },
        Read::OutOfBounds => { return true; },
    }
}

fn check_creation() -> bool {
    if error_code(zeros(-1, 0)) != 1 || error_code(zeros(0, -1)) != 1 { return false; }
    if error_code(zeros(2147483647, 2)) != 3 || error_code(zeros(268435456, 1)) != 3 { return false; }
    if error_code(zeros(3000000, 1)) != 5 { return false; }
    let input = [1.0, 2.0, 3.0];
    if error_code(from_slice(2, 2, &input)) != 1 { return false; }
    let created = zeros(1, 2);
    match created {
        MatrixResult::Ok(matrix) => {
            let mut output = matrix;
            let initial = [0.0, 0.0];
            if !same(&output, 1, 2, &initial) { return false; }
            if !missing(&output, -1, 0) || !missing(&output, 0, -1) || !missing(&output, 1, 0) || !missing(&output, 0, 2) { return false; }
            if set(&mut output, -1, 0, 1.0) || set(&mut output, 0, 2, 1.0) || set(&mut output, 2147483647, 2147483647, 1.0) { return false; }
            if !set(&mut output, 0, 1, 1.5) { return false; }
            let changed = [0.0, 1.5];
            return same(&output, 1, 2, &changed);
        },
        MatrixResult::Err(error) => { return false; },
    }
}

fn check_math() -> bool {
    let first = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let second = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let first_matrix = from_slice(2, 3, &first);
    let second_matrix = from_slice(3, 2, &second);
    match first_matrix {
        MatrixResult::Ok(left) => {
            match second_matrix {
                MatrixResult::Ok(right) => {
                    let multiplied = matmul(&left, &right);
                    match multiplied {
                        MatrixResult::Ok(product) => {
                            let expected = [58.0, 64.0, 139.0, 154.0];
                            if !same(&product, 2, 2, &expected) { return false; }
                        },
                        MatrixResult::Err(error) => { return false; },
                    }
                    let transposed = transpose(&left);
                    match transposed {
                        MatrixResult::Ok(matrix) => {
                            let expected = [1.0, 4.0, 2.0, 5.0, 3.0, 6.0];
                            if !same(&matrix, 3, 2, &expected) { return false; }
                        },
                        MatrixResult::Err(error) => { return false; },
                    }
                    if error_code(matmul(&left, &left)) != 2 { return false; }
                    return same(&left, 2, 3, &first) && same(&right, 3, 2, &second);
                },
                MatrixResult::Err(error) => { return false; },
            }
        },
        MatrixResult::Err(error) => { return false; },
    }
}

fn grown(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn check_transfer() -> bool {
    let mut input = new();
    if !grown(try_push(&mut input, -0.0)) || !grown(try_push(&mut input, 1.5)) { return false; }
    let refused = from_vector(input, 2, 2);
    match refused {
        FromVector::Ok(matrix) => { return false; },
        FromVector::InvalidShape(original) => {
            if vector_len(&original) != 2 { return false; }
            let shaped = from_vector(original, 1, 2);
            match shaped {
                FromVector::Ok(matrix) => {
                    let expected = [-0.0, 1.5];
                    if !same(&matrix, 1, 2, &expected) { return false; }
                    let restored = into_vector(matrix);
                    if vector_len(&restored) != 2 { return false; }
                    let first = vector_get(&restored, 0);
                    match first {
                        VectorRead::Value(value) => { return value.to_bits() == 9223372036854775808u64; },
                        VectorRead::OutOfBounds => { return false; },
                    }
                },
                FromVector::InvalidShape(rejected) => { return false; },
            }
        },
    }
}

fn check_empty() -> bool {
    let created = zeros(2147483647, 0);
    match created {
        MatrixResult::Ok(left) => {
            let transposed = transpose(&left);
            match transposed {
                MatrixResult::Ok(right) => {
                    if rows(&right) != 0 || columns(&right) != 2147483647 || !missing(&right, 0, 0) { return false; }
                    let product = matmul(&left, &right);
                    if error_code(product) != 3 { return false; }
                },
                MatrixResult::Err(error) => { return false; },
            }
        },
        MatrixResult::Err(error) => { return false; },
    }
    let first = zeros(2, 0);
    let second = zeros(0, 3);
    match first {
        MatrixResult::Ok(left) => {
            match second {
                MatrixResult::Ok(right) => {
                    let product = matmul(&left, &right);
                    match product {
                        MatrixResult::Ok(matrix) => {
                            let expected = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
                            return same(&matrix, 2, 3, &expected);
                        },
                        MatrixResult::Err(error) => { return false; },
                    }
                },
                MatrixResult::Err(error) => { return false; },
            }
        },
        MatrixResult::Err(error) => { return false; },
    }
}

fn main() -> i32 {
    if !check_creation() { return 1; }
    if !check_math() { return 2; }
    if !check_transfer() { return 3; }
    if !check_empty() { return 4; }
    return 0;
}
