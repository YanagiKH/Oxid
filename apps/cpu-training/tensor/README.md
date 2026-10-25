# Tensor operations

Checked CPU matrices and reverse-mode automatic differentiation, written in Oxid. Values use row-major `f64` storage. Operations return `TensorResult<T>` with `Ok(T)` or an explicit `Error`.

## Matrices

`from_slice(rows, columns, values)` checks the shape and copies a borrowed slice. `filled` initializes a matrix; `get` and `set` check coordinates; `copy_values` exports into a matching slice. `row_count` and `column_count` inspect the shape.

`matmul` performs matrix multiplication. `add` and `multiply` require equal shapes. `add_rows` broadcasts a single bias row across a matrix. `relu`, `mean` and `mean_squared_error` provide activation and reduction. Empty means and empty mean squared errors return `EmptyReduction`. A matrix product with a zero inner dimension contains zeros.

The storage fields are private. Shape errors, size overflow and allocation failure return errors. There are no general views, strides, dtype conversion or arbitrary broadcasting.

## Automatic differentiation

Create a graph with `graph::create`, copy tensors into it with `graph::input`, and connect variables using `matmul`, `add`, `add_rows`, `multiply`, `relu`, `mean` or `mse`. Variables belong to the graph that created them.

`backward` accepts a scalar loss. It resets previous gradients, visits only nodes that contribute to the loss, and accumulates contributions when an operand is shared. ReLU uses a derivative of zero at zero. `gradient` copies a variable's gradient into a tensor; `values` copies its forward values.

`sgd_update` checks the learning rate, shape and every candidate value before changing a tensor. Non-finite candidates leave the tensor unchanged. The model API validates all four parameter tensors before committing any update.

## Model

`model::initialize(inputs, hidden, outputs, seed)` creates a two-layer ReLU network with deterministic initialization. `predict`, `loss` and `train_step` run inference, compute mean squared error and perform one full-batch SGD update. `parameter` exports a parameter tensor and `update_count` reports successful updates.

`model::checkpoint::encode` and `decode` preserve parameter bits, RNG state and the update count. The decoder checks the format, exact size, dimensions, checksum and finite parameters before allocating a model. File I/O lives in the separate `checkpoint` package.
