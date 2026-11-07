use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::try_resize;
use alloc::bytes::truncate;
use crate::Tensor;
use crate::TensorResult;
use crate::Error;

pub struct Graph { nodes: ByteBuf, arena: ByteBuf }
pub struct Variable { index: i32 }

pub fn create() -> Graph { return Graph { nodes: new(), arena: new() }; }
pub fn count(graph: &Graph) -> i32 { return len(&*graph.nodes) / 32; }

fn word(bytes: &[u8], index: i32) -> i32 {
    let mut bits = 0u32;
    for offset in 0..4 {
        let byte = bytes[index * 4 + offset];
        bits = bits | (byte.to_u32_checked() << (offset * 8));
    }
    return bits.to_i32_checked();
}

fn put_word(bytes: &mut [u8], index: i32, number: i32) -> () {
    let bits = number.to_u32_checked();
    for offset in 0..4 {
        let byte = (bits >> (offset * 8)) & 255u32;
        bytes[index * 4 + offset] = byte.to_u8_checked();
    }
    return;
}

fn meta(graph: &Graph, index: i32, field: i32) -> i32 {
    return word(&*graph.nodes, (index - 1) * 8 + field);
}

fn live(graph: &Graph, variable: &Variable) -> bool {
    return variable.index > 0 && variable.index <= count(&*graph);
}

fn write_node(bytes: &mut [u8], index: i32, operation: i32, left: i32, right: i32, rows: i32, columns: i32, value_start: i32, gradient_start: i32) -> () {
    let start = (index - 1) * 8;
    put_word(&mut *bytes, start, operation);
    put_word(&mut *bytes, start + 1, left);
    put_word(&mut *bytes, start + 2, right);
    put_word(&mut *bytes, start + 3, rows);
    put_word(&mut *bytes, start + 4, columns);
    put_word(&mut *bytes, start + 5, value_start);
    put_word(&mut *bytes, start + 6, gradient_start);
    put_word(&mut *bytes, start + 7, rows * columns);
    return;
}

fn append(graph: &mut Graph, operation: i32, left: i32, right: i32, rows: i32, columns: i32) -> TensorResult<Variable>{
    let node_bytes = len(&*graph.nodes);
    let arena_elements = len(&*graph.arena) / 8;
    if rows < 0 || columns < 0 { return TensorResult::<Variable>:: Err(Error::NegativeShape); }
    if rows != 0 {
        if columns > 268435455 / rows { return TensorResult::<Variable>:: Err(Error::SizeOverflow); }
    }
    let elements = rows * columns;
    if elements > (268435455 - arena_elements) / 2 || node_bytes > 2147483615 {
        return TensorResult::<Variable>:: Err(Error::GraphCapacity);
    }
    let grown_nodes = try_resize(&mut *graph.nodes, node_bytes + 32, 0u8);
    match grown_nodes {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return TensorResult::<Variable>:: Err(Error::GraphCapacity); },
        GrowStatus::AllocationFailed => { return TensorResult::<Variable>:: Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return TensorResult::<Variable>:: Err(Error::MemoryLimit); }
    }
    let grown_arena = try_resize(&mut *graph.arena, (arena_elements + elements * 2) * 8, 0u8);
    match grown_arena {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => {
            truncate(&mut *graph.nodes, node_bytes);
            return TensorResult::<Variable>:: Err(Error::GraphCapacity);
        },
        GrowStatus::AllocationFailed => {
            truncate(&mut *graph.nodes, node_bytes);
            return TensorResult::<Variable>:: Err(Error::AllocationFailed);
        },
        GrowStatus::MemoryLimit => {
            truncate(&mut *graph.nodes, node_bytes);
            return TensorResult::<Variable>:: Err(Error::MemoryLimit);
        }
    }
    let index = node_bytes / 32 + 1;
    write_node(&mut *graph.nodes, index, operation, left, right, rows, columns, arena_elements, arena_elements + elements);
    return TensorResult::Ok(Variable { index: index });
}

fn copy_region(source: &[u8], target: &mut [u8], source_start: i32, target_start: i32, elements: i32) -> () {
    for element in 0..elements {
        let value = crate::codec::load(&*source, source_start + element);
        crate::codec::store(&mut *target, target_start + element, value);
    }
    return;
}

pub fn input(graph: &mut Graph, tensor: &Tensor) -> TensorResult<Variable>{
    if !crate::valid(&*tensor) { return TensorResult::<Variable>:: Err(Error::InvalidStorage); }
    let rows = tensor.rows;
    let columns = tensor.columns;
    let added = append(&mut *graph, 0, 0, 0, rows, columns);
    match added {
        TensorResult::Ok(variable) => {
            let start = meta(&*graph, variable.index, 5);
            copy_region(&*tensor.data, &mut *graph.arena, 0, start, rows * columns);
            return TensorResult::Ok(variable);
        },
        TensorResult::Err(error) => { return TensorResult::<Variable>:: Err(error); }
    }
}

pub fn values(graph: &Graph, variable: &Variable) -> TensorResult<Tensor>{
    return extract(&*graph, &*variable, 5);
}

pub fn gradient(graph: &Graph, variable: &Variable) -> TensorResult<Tensor>{
    return extract(&*graph, &*variable, 6);
}

fn extract(graph: &Graph, variable: &Variable, field: i32) -> TensorResult<Tensor>{
    if !live(&*graph, &*variable) { return TensorResult::<Tensor>:: Err(Error::InvalidNode); }
    let rows = meta(&*graph, variable.index, 3);
    let columns = meta(&*graph, variable.index, 4);
    let start = meta(&*graph, variable.index, field);
    let mut output = crate::empty();
    let made = crate::filled(rows, columns, 0.0);
    match made {
        TensorResult::Ok(tensor) => { output = tensor; },
        TensorResult::Err(error) => { return TensorResult::<Tensor>:: Err(error); }
    }
    copy_region(&*graph.arena, &mut output.data, start, 0, rows * columns);
    return TensorResult::Ok(output);
}

pub fn add(graph: &mut Graph, left: &Variable, right: &Variable) -> TensorResult<Variable>{
    return pair(&mut *graph, &*left, &*right, 1);
}

pub fn multiply(graph: &mut Graph, left: &Variable, right: &Variable) -> TensorResult<Variable>{
    return pair(&mut *graph, &*left, &*right, 6);
}

pub fn add_rows(graph: &mut Graph, left: &Variable, right: &Variable) -> TensorResult<Variable>{
    return pair(&mut *graph, &*left, &*right, 5);
}

pub fn matmul(graph: &mut Graph, left: &Variable, right: &Variable) -> TensorResult<Variable>{
    return pair(&mut *graph, &*left, &*right, 2);
}

fn pair(graph: &mut Graph, left: &Variable, right: &Variable, operation: i32) -> TensorResult<Variable>{
    if !live(&*graph, &*left) || !live(&*graph, &*right) { return TensorResult::<Variable>:: Err(Error::InvalidNode); }
    let left_rows = meta(&*graph, left.index, 3);
    let left_columns = meta(&*graph, left.index, 4);
    let right_rows = meta(&*graph, right.index, 3);
    let right_columns = meta(&*graph, right.index, 4);
    let mut output_columns = left_columns;
    if operation == 2 {
        if left_columns != right_rows { return TensorResult::<Variable>:: Err(Error::ShapeMismatch); }
        output_columns = right_columns;
    } else {
        if operation == 5 {
            if right_rows != 1 || left_columns != right_columns { return TensorResult::<Variable>:: Err(Error::ShapeMismatch); }
        } else {
            if left_rows != right_rows || left_columns != right_columns { return TensorResult::<Variable>:: Err(Error::ShapeMismatch); }
        }
    }
    let added = append(&mut *graph, operation, left.index, right.index, left_rows, output_columns);
    match added {
        TensorResult::Ok(variable) => {
            let a = meta(&*graph, left.index, 5);
            let b = meta(&*graph, right.index, 5);
            let output_start = meta(&*graph, variable.index, 5);
            if operation == 2 {
                matrix_kernel(&mut *graph.arena, a, b, output_start, left_rows, left_columns, right_columns);
            } else {
                pair_kernel(&mut *graph.arena, a, b, output_start, left_rows * left_columns, left_columns, operation);
            }
            return TensorResult::Ok(variable);
        },
        TensorResult::Err(error) => { return TensorResult::<Variable>:: Err(error); }
    }
}

fn matrix_kernel(arena: &mut [u8], a: i32, b: i32, output_start: i32, rows: i32, inner: i32, columns: i32) -> () {
    for row in 0..rows {
        for column in 0..columns {
            let mut sum = 0.0;
            for index in 0..inner {
                let first = crate::codec::load(&*arena, a + row * inner + index);
                let second = crate::codec::load(&*arena, b + index * columns + column);
                sum = sum + first * second;
            }
            crate::codec::store(&mut *arena, output_start + row * columns + column, sum);
        }
    }
    return;
}

fn pair_kernel(arena: &mut [u8], a: i32, b: i32, output_start: i32, elements: i32, columns: i32, operation: i32) -> () {
    for element in 0..elements {
        let first = crate::codec::load(&*arena, a + element);
        let mut second_index = element;
        if operation == 5 { second_index = element % columns; }
        let second = crate::codec::load(&*arena, b + second_index);
        let mut number = first + second;
        if operation == 6 { number = first * second; }
        crate::codec::store(&mut *arena, output_start + element, number);
    }
    return;
}

pub fn relu(graph: &mut Graph, source: &Variable) -> TensorResult<Variable>{
    if !live(&*graph, &*source) { return TensorResult::<Variable>:: Err(Error::InvalidNode); }
    let rows = meta(&*graph, source.index, 3);
    let columns = meta(&*graph, source.index, 4);
    let added = append(&mut *graph, 3, source.index, 0, rows, columns);
    match added {
        TensorResult::Ok(variable) => {
            let a = meta(&*graph, source.index, 5);
            let output_start = meta(&*graph, variable.index, 5);
            rectifier_kernel(&mut *graph.arena, a, output_start, rows * columns);
            return TensorResult::Ok(variable);
        },
        TensorResult::Err(error) => { return TensorResult::<Variable>:: Err(error); }
    }
}

fn rectifier_kernel(arena: &mut [u8], a: i32, output_start: i32, elements: i32) -> () {
    for element in 0..elements {
        let mut number = crate::codec::load(&*arena, a + element);
        if number < 0.0 { number = 0.0; }
        crate::codec::store(&mut *arena, output_start + element, number);
    }
    return;
}

pub fn mse(graph: &mut Graph, left: &Variable, right: &Variable) -> TensorResult<Variable>{
    if !live(&*graph, &*left) || !live(&*graph, &*right) { return TensorResult::<Variable>:: Err(Error::InvalidNode); }
    let rows = meta(&*graph, left.index, 3);
    let columns = meta(&*graph, left.index, 4);
    if rows != meta(&*graph, right.index, 3) || columns != meta(&*graph, right.index, 4) { return TensorResult::<Variable>:: Err(Error::ShapeMismatch); }
    if rows * columns == 0 { return TensorResult::<Variable>:: Err(Error::EmptyReduction); }
    let added = append(&mut *graph, 4, left.index, right.index, 1, 1);
    match added {
        TensorResult::Ok(variable) => {
            let a = meta(&*graph, left.index, 5);
            let b = meta(&*graph, right.index, 5);
            let output_start = meta(&*graph, variable.index, 5);
            mse_kernel(&mut *graph.arena, a, b, output_start, rows * columns);
            return TensorResult::Ok(variable);
        },
        TensorResult::Err(error) => { return TensorResult::<Variable>:: Err(error); }
    }
}

fn mse_kernel(arena: &mut [u8], a: i32, b: i32, output_start: i32, elements: i32) -> () {
    let mut sum = 0.0;
    for element in 0..elements {
        let first = crate::codec::load(&*arena, a + element);
        let second = crate::codec::load(&*arena, b + element);
        let delta = first - second;
        sum = sum + delta * delta;
    }
    crate::codec::store(&mut *arena, output_start, sum / elements.to_f64());
    return;
}

pub fn mean(graph: &mut Graph, source: &Variable) -> TensorResult<Variable>{
    if !live(&*graph, &*source) { return TensorResult::<Variable>:: Err(Error::InvalidNode); }
    let rows = meta(&*graph, source.index, 3);
    let columns = meta(&*graph, source.index, 4);
    if rows * columns == 0 { return TensorResult::<Variable>:: Err(Error::EmptyReduction); }
    let added = append(&mut *graph, 7, source.index, 0, 1, 1);
    match added {
        TensorResult::Ok(variable) => {
            let a = meta(&*graph, source.index, 5);
            let output_start = meta(&*graph, variable.index, 5);
            reduction_kernel(&mut *graph.arena, a, output_start, rows * columns);
            return TensorResult::Ok(variable);
        },
        TensorResult::Err(error) => { return TensorResult::<Variable>:: Err(error); }
    }
}

fn reduction_kernel(arena: &mut [u8], a: i32, output_start: i32, elements: i32) -> () {
    let mut sum = 0.0;
    for element in 0..elements { sum = sum + crate::codec::load(&*arena, a + element); }
    crate::codec::store(&mut *arena, output_start, sum / elements.to_f64());
    return;
}

fn flag(flags: &[u8], index: i32) -> bool { return flags[index - 1] != 0u8; }

fn reachable(nodes: &[u8], flags: &mut [u8], root: i32) -> () {
    flags[root - 1] = 1u8;
    let total = flags.len();
    for offset in 0..total {
        let index = total - offset;
        if flags[index - 1] != 0u8 {
            let left = word(&*nodes, (index - 1) * 8 + 1);
            let right = word(&*nodes, (index - 1) * 8 + 2);
            if left > 0 { flags[left - 1] = 1u8; }
            if right > 0 { flags[right - 1] = 1u8; }
        }
    }
    return;
}

fn zero_region(arena: &mut [u8], start: i32, elements: i32) -> () {
    for element in 0..elements { crate::codec::store(&mut *arena, start + element, 0.0); }
    return;
}

fn accumulate(arena: &mut [u8], index: i32, delta: f64) -> () {
    let previous = crate::codec::load(&*arena, index);
    crate::codec::store(&mut *arena, index, previous + delta);
    return;
}

pub fn backward(graph: &mut Graph, root: &Variable) -> TensorResult<()>{
    if !live(&*graph, &*root) { return TensorResult::<()>:: Err(Error::InvalidNode); }
    if meta(&*graph, root.index, 7) != 1 { return TensorResult::<()>:: Err(Error::ShapeMismatch); }
    let total = count(&*graph);
    let mut flags = new();
    let allocated = try_resize(&mut flags, total, 0u8);
    match allocated {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return TensorResult::<()>:: Err(Error::GraphCapacity); },
        GrowStatus::AllocationFailed => { return TensorResult::<()>:: Err(Error::AllocationFailed); },
        GrowStatus::MemoryLimit => { return TensorResult::<()>:: Err(Error::MemoryLimit); }
    }
    reachable(&*graph.nodes, &mut flags, root.index);
    for offset in 0..total {
        let index = offset + 1;
        let base = meta(&*graph, index, 6);
        let elements = meta(&*graph, index, 7);
        zero_region(&mut *graph.arena, base, elements);
    }
    let seed = meta(&*graph, root.index, 6);
    crate::codec::store(&mut *graph.arena, seed, 1.0);
    for offset in 0..total {
        let index = total - offset;
        if !flag(&flags, index) { continue; }
        let operation = meta(&*graph, index, 0);
        if operation == 0 { continue; }
        let left = meta(&*graph, index, 1);
        let right = meta(&*graph, index, 2);
        let a = meta(&*graph, left, 5);
        let da = meta(&*graph, left, 6);
        let output_gradient = meta(&*graph, index, 6);
        let rows = meta(&*graph, left, 3);
        let columns = meta(&*graph, left, 4);
        if operation == 3 {
            rectifier_backward(&mut *graph.arena, a, da, output_gradient, rows * columns);
        } else {
            if operation == 7 {
                reduction_backward(&mut *graph.arena, da, output_gradient, rows * columns);
            } else {
                let b = meta(&*graph, right, 5);
                let db = meta(&*graph, right, 6);
                if operation == 2 {
                    let output_columns = meta(&*graph, right, 4);
                    matrix_backward(&mut *graph.arena, a, b, da, db, output_gradient, rows, columns, output_columns);
                } else {
                    if operation == 4 {
                        mse_backward(&mut *graph.arena, a, b, da, db, output_gradient, rows * columns);
                    } else {
                        pair_backward(&mut *graph.arena, a, b, da, db, output_gradient, rows * columns, columns, operation);
                    }
                }
            }
        }
    }
    return TensorResult::Ok(());
}

fn rectifier_backward(arena: &mut [u8], a: i32, da: i32, output_gradient: i32, elements: i32) -> () {
    for element in 0..elements {
        let number = crate::codec::load(&*arena, a + element);
        let upstream = crate::codec::load(&*arena, output_gradient + element);
        let mut derivative = 0.0;
        if number > 0.0 { derivative = 1.0; }
        if number != number { derivative = number; }
        accumulate(&mut *arena, da + element, upstream * derivative);
    }
    return;
}

fn reduction_backward(arena: &mut [u8], da: i32, output_gradient: i32, elements: i32) -> () {
    let upstream = crate::codec::load(&*arena, output_gradient) / elements.to_f64();
    for element in 0..elements { accumulate(&mut *arena, da + element, upstream); }
    return;
}

fn matrix_backward(arena: &mut [u8], a: i32, b: i32, da: i32, db: i32, output_gradient: i32, rows: i32, inner: i32, columns: i32) -> () {
    for row in 0..rows {
        for column in 0..columns {
            let upstream = crate::codec::load(&*arena, output_gradient + row * columns + column);
            for index in 0..inner {
                let first = crate::codec::load(&*arena, a + row * inner + index);
                let second = crate::codec::load(&*arena, b + index * columns + column);
                accumulate(&mut *arena, da + row * inner + index, upstream * second);
                accumulate(&mut *arena, db + index * columns + column, first * upstream);
            }
        }
    }
    return;
}

fn mse_backward(arena: &mut [u8], a: i32, b: i32, da: i32, db: i32, output_gradient: i32, elements: i32) -> () {
    let upstream = crate::codec::load(&*arena, output_gradient);
    let scale = 2.0 * upstream / elements.to_f64();
    for element in 0..elements {
        let first = crate::codec::load(&*arena, a + element);
        let second = crate::codec::load(&*arena, b + element);
        let delta = scale * (first - second);
        accumulate(&mut *arena, da + element, delta);
        accumulate(&mut *arena, db + element, -delta);
    }
    return;
}

fn pair_backward(arena: &mut [u8], a: i32, b: i32, da: i32, db: i32, output_gradient: i32, elements: i32, columns: i32, operation: i32) -> () {
    for element in 0..elements {
        let mut second_index = element;
        if operation == 5 { second_index = element % columns; }
        let upstream = crate::codec::load(&*arena, output_gradient + element);
        let mut left_delta = upstream;
        let mut right_delta = upstream;
        if operation == 6 {
            let first = crate::codec::load(&*arena, a + element);
            let second = crate::codec::load(&*arena, b + second_index);
            left_delta = upstream * second;
            right_delta = upstream * first;
        }
        accumulate(&mut *arena, da + element, left_delta);
        accumulate(&mut *arena, db + second_index, right_delta);
    }
    return;
}

fn finite(number: f64) -> bool {
    let bits = number.to_bits();
    return ((bits >> 52) & 2047u64) != 2047u64;
}

pub fn validate_sgd_update(target: &Tensor, graph: &Graph, variable: &Variable, rate: f64) -> TensorResult<()>{
    if !live(&*graph, &*variable) { return TensorResult::<()>:: Err(Error::InvalidNode); }
    if !crate::valid(&*target) { return TensorResult::<()>:: Err(Error::InvalidStorage); }
    if target.rows != meta(&*graph, variable.index, 3) || target.columns != meta(&*graph, variable.index, 4) {
        return TensorResult::<()>:: Err(Error::ShapeMismatch);
    }
    if !finite(rate) || rate < 0.0 { return TensorResult::<()>:: Err(Error::InvalidRate); }
    let start = meta(&*graph, variable.index, 6);
    let elements = target.rows * target.columns;
    if !step_valid(&*target.data, &*graph.arena, start, elements, rate) {
        return TensorResult::<()>:: Err(Error::NonFinite);
    }
    return TensorResult::Ok(());
}

pub fn sgd_update(target: &mut Tensor, graph: &Graph, variable: &Variable, rate: f64) -> TensorResult<()>{
    let checked = validate_sgd_update(&*target, &*graph, &*variable, rate);
    match checked { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); } }
    let start = meta(&*graph, variable.index, 6);
    let elements = target.rows * target.columns;
    step_commit(&mut *target.data, &*graph.arena, start, elements, rate);
    return TensorResult::Ok(());
}

pub fn value_at(graph: &Graph, variable: &Variable, row: i32, column: i32) -> TensorResult<f64>{
    if !live(&*graph, &*variable) { return TensorResult::<f64>:: Err(Error::InvalidNode); }
    let rows = meta(&*graph, variable.index, 3);
    let columns = meta(&*graph, variable.index, 4);
    if row < 0 || column < 0 || row >= rows || column >= columns { return TensorResult::<f64>:: Err(Error::Bounds); }
    let start = meta(&*graph, variable.index, 5);
    return TensorResult::Ok(crate::codec::load(&*graph.arena, start + row * columns + column));
}

pub fn gradient_at(graph: &Graph, variable: &Variable, row: i32, column: i32) -> TensorResult<f64>{
    if !live(&*graph, &*variable) { return TensorResult::<f64>:: Err(Error::InvalidNode); }
    let rows = meta(&*graph, variable.index, 3);
    let columns = meta(&*graph, variable.index, 4);
    if row < 0 || column < 0 || row >= rows || column >= columns { return TensorResult::<f64>:: Err(Error::Bounds); }
    let start = meta(&*graph, variable.index, 6);
    return TensorResult::Ok(crate::codec::load(&*graph.arena, start + row * columns + column));
}

fn step_valid(parameters: &[u8], gradients: &[u8], start: i32, elements: i32, rate: f64) -> bool {
    for element in 0..elements {
        let parameter = crate::codec::load(&*parameters, element);
        let derivative = crate::codec::load(&*gradients, start + element);
        let candidate = parameter - rate * derivative;
        if !finite(candidate) { return false; }
    }
    return true;
}

fn step_commit(parameters: &mut [u8], gradients: &[u8], start: i32, elements: i32, rate: f64) -> () {
    for element in 0..elements {
        let parameter = crate::codec::load(&*parameters, element);
        let derivative = crate::codec::load(&*gradients, start + element);
        crate::codec::store(&mut *parameters, element, parameter - rate * derivative);
    }
    return;
}
