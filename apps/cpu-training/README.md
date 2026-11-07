# CPU training

Train a small neural network with Oxid tensors, automatic differentiation and SGD, Adam or AdamW. The executable runs on the CPU and saves checkpoints for continued training.

```sh
cd apps/cpu-training
oxid test --workspace
oxid build --package cpu-training
./trainer/.oxid/bin/cpu-training train xor 2000
./trainer/.oxid/bin/cpu-training resume xor 500
```

To use Adam, choose a new checkpoint prefix:

```sh
./trainer/.oxid/bin/cpu-training train xor-adam 2000 adam
./trainer/.oxid/bin/cpu-training resume xor-adam 500
```

The example learns XOR with a 2 → 8 → 1 network, ReLU activation, mean squared error and a fixed seed. It prints the update count, loss and checkpoint path. Loss is shown with six significant digits; checkpoints preserve the original floating-point bits.

| Optimizer | Learning rate | Other defaults |
| --- | ---: | --- |
| `sgd` (default) | 0.05 | Plain SGD |
| `adam` | 0.01 | Betas 0.9 and 0.999; epsilon 0.00000001 |
| `adamw` | 0.01 | Adam settings with decoupled weight decay 0.01 |

`train` starts a new model and refuses an existing checkpoint prefix. `resume` continues the newest valid checkpoint. Steps must be between 1 and 1,000,000; the default is 2,000. Run without arguments or with `--help` for usage.

## Checkpoints

For the prefix `xor`, the application alternates between `xor.a` and `xor.b`, saving every 25 updates and at completion. A saved model includes weights, biases, RNG state and the update count. Adam and AdamW checkpoints also preserve optimizer settings, moments, bias-correction powers and the optimizer step count. `resume` restores these values automatically; it does not accept a replacement optimizer.

Existing fixed-rate SGD model checkpoints remain readable. Default SGD training continues to use that format. The tensor library also provides a complete session format for custom SGD settings, clipping and learning-rate schedules.

An interrupted or damaged latest file falls back to the other valid file. Missing files and failed integrity checks permit recovery; permission, allocation and other I/O errors stop the command. Use one trainer per checkpoint prefix.

## Packages

- [`tensor`](tensor/README.md) provides checked row-major `f64` matrices, reverse-mode automatic differentiation and the configurable two-layer model.
- `checkpoint` reads and writes models or complete training sessions, verifies checksums and selects a recovery file.
- `trainer` supplies the command-line example and workspace tests.

This application is experimental and uses the Linux x86_64／ARM64 native backend with LLVM 19.1.7 and hosted execution. Compiler and memory limits apply. It uses full batches and row-major `f64` tensors; general layouts, other dtypes and mini-batch data pipelines are not available.
