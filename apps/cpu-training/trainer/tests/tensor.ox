use crate::tensor::TensorResult;
use crate::tensor::Tensor;
use crate::tensor::Error;

fn error_is_memory(error: Error) -> bool {
    match error {
        Error::MemoryLimit => { return true; },
        Error::NegativeShape => { return false; }, Error::SizeOverflow => { return false; },
        Error::ShapeMismatch => { return false; }, Error::Bounds => { return false; },
        Error::AllocationFailed => { return false; }, Error::InvalidStorage => { return false; },
        Error::EmptyReduction => { return false; }, Error::InvalidNode => { return false; },
        Error::GraphCapacity => { return false; }, Error::NonFinite => { return false; },
        Error::InvalidRate => { return false; }, Error::StepOverflow => { return false; },
        Error::InvalidCheckpoint => { return false; }, Error::IoError(code) => { return false; },
        Error::WriteZero => { return false; }
    }
}

fn same_values(tensor: &Tensor, expected: &[f64]) -> bool {
    let columns = crate::tensor::column_count(&*tensor);
    if crate::tensor::row_count(&*tensor) * columns != expected.len() { return false; }
    for index in 0..expected.len() {
        let read = crate::tensor::get(&*tensor, index / columns, index % columns);
        match read { TensorResult::Ok(number) => { if number != expected[index] { return false; } }, TensorResult::Err(error) => { return false; } }
    }
    return true;
}

fn matrix_product(left: &Tensor) -> i32 {
    let numbers = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let right = crate::tensor::from_slice(3, 2, &numbers);
    match right {
        TensorResult::Ok(rhs) => {
            let product = crate::tensor::matmul(&*left, &rhs);
            match product { TensorResult::Ok(tensor) => { let expected = [58.0, 64.0, 139.0, 154.0]; if !same_values(&tensor, &expected) { return 3; } return 0; }, TensorResult::Err(error) => { return 2; } }
        },
        TensorResult::Err(error) => { return 1; }
    }
}

fn matrices() -> i32 {
    let numbers = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let left = crate::tensor::from_slice(2, 3, &numbers);
    match left { TensorResult::Ok(tensor) => { return matrix_product(&tensor); }, TensorResult::Err(error) => { return 4; } }
}

fn repeat_backward(tape: &mut crate::tensor::graph::Graph, variable: &crate::tensor::graph::Variable, root: &crate::tensor::graph::Variable) -> i32 {
    for iteration in 0..2 {
        let result = crate::tensor::graph::backward(&mut *tape, &*root);
        match result { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return 5; } }
        let read = crate::tensor::graph::gradient(&*tape, &*variable);
        match read { TensorResult::Ok(tensor) => { let expected = [1.5, -2.0]; if !same_values(&tensor, &expected) { return 6; } }, TensorResult::Err(error) => { return 7; } }
    }
    return 0;
}

fn square_graph(tensor: &Tensor) -> i32 {
    let mut tape = crate::tensor::graph::create();
    let input = crate::tensor::graph::input(&mut tape, &*tensor);
    match input {
        TensorResult::Ok(variable) => {
            let product = crate::tensor::graph::multiply(&mut tape, &variable, &variable);
            match product {
                TensorResult::Ok(squared) => {
                    let reduced = crate::tensor::graph::mean(&mut tape, &squared);
                    match reduced { TensorResult::Ok(root) => { return repeat_backward(&mut tape, &variable, &root); }, TensorResult::Err(error) => { return 8; } }
                },
                TensorResult::Err(error) => { return 9; }
            }
        },
        TensorResult::Err(error) => { return 10; }
    }
}

fn gradients() -> i32 {
    let numbers = [1.5, -2.0];
    let made = crate::tensor::from_slice(1, 2, &numbers);
    match made { TensorResult::Ok(tensor) => { return square_graph(&tensor); }, TensorResult::Err(error) => { return 11; } }
}

fn failed_graph_input(tape: &mut crate::tensor::graph::Graph) -> i32 {
    let made = crate::tensor::filled(1100000, 1, 0.0);
    match made {
        TensorResult::Ok(tensor) => {
            let added = crate::tensor::graph::input(&mut *tape, &tensor);
            match added { TensorResult::Ok(variable) => { return 12; }, TensorResult::Err(error) => { if !error_is_memory(error) { return 13; } } }
            return 0;
        },
        TensorResult::Err(error) => { return 14; }
    }
}

fn allocation_rollback() -> i32 {
    let mut tape = crate::tensor::graph::create();
    let status = failed_graph_input(&mut tape);
    if status != 0 { return status; }
    if crate::tensor::graph::count(&tape) != 0 { return 15; }
    let made = crate::tensor::filled(1, 1, 42.0);
    match made {
        TensorResult::Ok(tensor) => {
            let added = crate::tensor::graph::input(&mut tape, &tensor);
            match added {
                TensorResult::Ok(variable) => {
                    if crate::tensor::graph::count(&tape) != 1 { return 16; }
                    let read = crate::tensor::graph::value_at(&tape, &variable, 0, 0);
                    match read { TensorResult::Ok(number) => { if number != 42.0 { return 17; } return 0; }, TensorResult::Err(error) => { return 18; } }
                },
                TensorResult::Err(error) => { return 19; }
            }
        },
        TensorResult::Err(error) => { return 20; }
    }
}

fn main() -> i32 {
    let first = matrices(); if first != 0 { return first; }
    let second = gradients(); if second != 0 { return second; }
    return allocation_rollback();
}
