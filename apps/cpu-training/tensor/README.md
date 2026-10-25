# Tensor operations

Checked CPU matrices and reverse-mode automatic differentiation, written in Oxid. Values use row-major `f64` storage. Operations return `TensorResult<T>` with `Ok(T)` or an explicit `Error`.

## Matrices

`from_slice(rows, columns, values)` checks the shape and copies a borrowed slice. `filled` initializes a matrix; `get` and `set` check coordinates; `copy_values` exports into a matching slice. `row_count` and `column_count` inspect the shape.

`matmul` performs matrix multiplication. `add` and `multiply` require equal shapes. `add_rows` broadcasts a single bias row across a matrix. `relu`, `mean` and `mean_squared_error` provide activation and reduction. Empty means and empty mean squared errors return `EmptyReduction`. A matrix product with a zero inner dimension contains zeros.

The storage fields are private. Shape errors, size overflow and allocation failure return errors. There are no general views, strides, dtype conversion or arbitrary broadcasting.

## Automatic differentiation

Create a graph with `graph::create`, copy tensors into it with `graph::input`, and connect variables using `matmul`, `add`, `add_rows`, `multiply`, `relu`, `mean` or `mse`. Variables belong to the graph that created them.

`backward` accepts a scalar loss. It resets previous gradients, visits only nodes that contribute to the loss, and accumulates contributions when an operand is shared. ReLU uses a derivative of zero at zero. `gradient` copies a variable's gradient into a tensor; `gradient_at` reads one gradient value; `values` copies forward values.

`sgd_update` checks the learning rate, shape and every candidate value before changing a tensor. Non-finite candidates leave the tensor unchanged. The model API validates all four parameter tensors before committing any update.

## Model

`model::initialize(inputs, hidden, outputs, seed)` creates a two-layer ReLU network with deterministic initialization. `predict`, `loss` and `train_step` run inference, compute mean squared error and perform one full-batch SGD update. `parameter` exports a parameter tensor and `update_count` reports successful updates.

`model::checkpoint::encode` and `decode` preserve parameter bits, RNG state and the update count. The decoder checks the format, exact size, dimensions, checksum and finite parameters before allocating a model. File I/O lives in the separate `checkpoint` package.

## Optimizers

`optimizer::sgd`, `adam` and `adamw` create configurations. Pass a configuration and the parameter shape to `optimizer::initialize`, then call `optimizer::step` with mutable parameters and a matching gradient tensor. AdamW applies weight decay separately from the gradient update.

`set_clip_norm` limits the global gradient norm; zero disables clipping. `set_step_decay` multiplies the base learning rate by a factor after each configured number of successful updates. `learning_rate` reports the rate for the next update; `set_base_learning_rate` changes its base. Invalid settings return an error.

An update checks every candidate parameter and moment before committing any change. Rejected shapes, non-finite values and counter overflow preserve parameters and optimizer history. A failed update does not advance the schedule. `step_count`, `first_moment` and `second_moment` inspect progress; plain SGD has no moment tensors.

`model::parameters` exports all model parameters in layer order. `set_parameters` validates an entire replacement before changing the model. `train_with_optimizer` computes the full model gradient and applies the selected optimizer while preserving the same failure guarantees.

`optimizer::checkpoint::encode` and `decode` preserve settings, moments, powers, schedule and counter. `model::session::encode` stores a model with its optimizer; `decode` accepts complete sessions and existing fixed-rate SGD model checkpoints. Model and optimizer counters are independent, so importing or resetting an optimizer does not change the model's update history.

`checkpoint::training::save`, `load` and `load_latest` provide file I/O and two-file recovery for complete sessions. Save each updated session to the other slot, preserving the previous valid file until the write completes.

## Data and batches

`data::create(inputs, targets)` takes ownership of two finite matrices with equal nonzero row counts and nonzero column counts. `row_count`, `feature_count`, `target_count`, `input_at` and `target_at` provide read access.

`data::initialize_sampler(dataset, batch_size, shuffle, seed)` prepares an epoch order. Every row appears once per epoch, including the final partial batch. A batch size larger than the row count selects the whole dataset. A zero seed selects a fixed nonzero default; the same inputs and seed reproduce the same order.

`prepare` copies the next pair of batches without advancing the sampler. `next_batch` advances only after both matrices are copied successfully. `train_next` advances model, optimizer and sampler only after the complete training update succeeds. Rejected shapes, non-finite calculations, allocation failures and counter overflow preserve their previous states. Epoch shuffling allocates no additional memory.

`batch_count`, `epoch`, `position`, `next_size` and `row_index` inspect the next batch. The batch counter records consumed batches, independently of model and optimizer update counters. Shuffle uses xorshift32 and rejection sampling over its nonzero output range, followed by Fisher–Yates from an identity order at each epoch.

`data::checkpoint` encodes paired matrices; `data::sampler_checkpoint` preserves permutation, RNG and progress. Sampler decoding checks dimensions, reserved fields, exact lengths, checksum, unique in-range row indices and consistent epoch/cursor/counter values. Unshuffled states require identity order.

`data::session` stores the model, optimizer, dataset and sampler together. It validates their dimensions and embedded formats. `checkpoint::pipeline` adds dataset and session files plus two-slot recovery. See [minibatch training](../../minibatch-training/README.md) for commands and data export.
