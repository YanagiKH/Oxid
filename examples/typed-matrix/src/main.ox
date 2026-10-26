use crate::matrix::Read;
use crate::matrix::Matrix;
use crate::matrix::MatrixResult;
use crate::matrix::from_slice;
use crate::matrix::matmul;
use crate::matrix::get;

fn multiply_last(left: Matrix, right: Matrix) -> f64 {
    let multiplied = matmul(&left, &right);
    match multiplied {
        MatrixResult::Ok(product) => {
            let found = get(&product, 1, 1);
            match found {
                Read::Value(value) => { return value; },
                Read::OutOfBounds => { return -1.0; },
            }
        },
        MatrixResult::Err(error) => { return -1.0; },
    }
}

fn main() -> f64 {
    let first = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let second = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let first_matrix = from_slice(2, 3, &first);
    let second_matrix = from_slice(3, 2, &second);
    match first_matrix {
        MatrixResult::Ok(left) => {
            match second_matrix {
                MatrixResult::Ok(right) => { return multiply_last(left, right); },
                MatrixResult::Err(error) => { return -1.0; },
            }
        },
        MatrixResult::Err(error) => { return -1.0; },
    }
}
