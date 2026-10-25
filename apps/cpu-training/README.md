# CPU training

Train a small neural network with Oxid tensor operations, automatic differentiation and SGD. The executable runs on the CPU and saves checkpoints for continued training.

```sh
cd apps/cpu-training
oxid test --workspace
oxid build --package cpu-training
./trainer/.oxid/bin/cpu-training train xor 2000
./trainer/.oxid/bin/cpu-training resume xor 500
```

The example learns XOR with a 2 → 8 → 1 network, ReLU activation, mean squared error, a learning rate of `0.05` and a fixed seed. It prints the update count, loss and checkpoint path. Loss is shown with six significant digits; checkpoints preserve the original floating-point bits.

`train` starts a new model and refuses an existing checkpoint prefix. `resume` continues the newest valid checkpoint. Steps must be between 1 and 1,000,000; the default is 2,000. Run without arguments or with `--help` for usage.

## Checkpoints

For the prefix `xor`, the application alternates between `xor.a` and `xor.b`, saving every 25 updates and at completion. A saved model includes weights, biases, RNG state and the update count. The example uses full batches and fixed-rate SGD, so these fields describe its training state.

An interrupted or damaged latest file falls back to the other valid file. Missing files and failed integrity checks permit recovery; permission, allocation and other I/O errors stop the command. Use one trainer per checkpoint prefix.

## Packages

- [`tensor`](tensor/README.md) provides checked row-major `f64` matrices, reverse-mode automatic differentiation and the configurable two-layer model.
- `checkpoint` reads and writes the versioned model format, verifies checksums and selects a recovery file.
- `trainer` supplies the command-line example and workspace tests.

This application is experimental and uses the Linux x86_64／ARM64 native backend with LLVM 19.1.7 and hosted execution. Existing compiler and memory limits apply. General tensor layouts, other dtypes, Adam, mini-batch data pipelines and larger training workloads remain separate work.
