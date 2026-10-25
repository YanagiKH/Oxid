# Minibatch training

Train your own paired data with SGD, Adam or AdamW. Each checkpoint saves the model, optimizer history, dataset and next batch, so resuming preserves the exact training sequence.

```sh
oxid build --profile release
.oxid/bin/minibatch-training train data.oxdata experiment 2000 3 adam 1
.oxid/bin/minibatch-training resume experiment 1000
```

The arguments after `train` are the dataset, checkpoint prefix, update count, batch size, optional optimizer and optional shuffle seed. Updates range from 1 to 1,000,000. The default optimizer is SGD; the default shuffle seed is 42. A final partial batch is included in each epoch, and a batch size larger than the dataset uses all rows.

SGD uses a learning rate of 0.05. Adam and AdamW use 0.01; AdamW also uses weight decay of 0.01. The model has one hidden ReLU layer with eight units and a linear output layer, trained with mean squared error. Input and output dimensions come from the dataset.

## Prepare data

An Oxid program can create paired matrices with `tensor::from_slice`, pass them to `tensor::data::create`, and write the result with `checkpoint::pipeline::save_dataset`. Inputs and targets must have the same nonzero row count, nonzero column counts and finite `f64` values.

This Python example exports the four XOR rows in the same binary format:

```python
import struct
import zlib
from pathlib import Path

inputs = [0., 0., 0., 1., 1., 0., 1., 1.]
targets = [0., 1., 1., 0.]
values = inputs + targets
body = b"OXDS0001" + struct.pack("<IIIIQ", 4, 2, 1, 0, len(values) * 8)
body += struct.pack(f"<{len(values)}d", *values)
Path("data.oxdata").write_bytes(body + struct.pack("<I", zlib.crc32(body)))
```

All integers and values use little-endian encoding. The header stores rows, input columns, target columns, a reserved zero field and payload length. Row-major inputs precede row-major targets; the final four bytes contain CRC-32 of all preceding bytes. Invalid dimensions, lengths, checksums and non-finite values are rejected.

## Resume and recovery

Checkpoints alternate between `experiment.a` and `experiment.b` every 25 successful updates and at completion. `resume` selects the valid file with the larger batch count. A missing or malformed file permits recovery from the other slot. During recovery, I/O and memory failures stop before changing either file. Start with a new prefix to avoid overwriting an existing experiment.

The original dataset file is unnecessary after the first checkpoint. Resume restores data values, shuffle order, RNG state, epoch position and optimizer settings from that checkpoint. Model, optimizer and sampler counters are stored independently.

Datasets are held in memory. The current compiler heap limit is 16 MiB, and encoded data or sessions cannot exceed 16 MiB. Dataset, model, batch, gradient and checkpoint buffers share that budget. This application does not stream datasets larger than memory.
