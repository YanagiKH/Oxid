# Streaming training

Train a small neural network from a binary dataset without loading the whole file into memory. Oxid reads one batch at a time and repeats the file for each epoch.

```sh
cd apps/streaming-training
oxid run -- data/sample.oxds 30 4
```

The sample has eight rows. Thirty epochs with batches of four produce 60 updates and reduce mean squared error from about 3.68 to 0.0121.

Build a native executable:

```sh
oxid build --profile release
.oxid/bin/streaming-training data/sample.oxds 30 4
```

Use `--help` for usage. Epochs must be between 1 and 1,000,000; batch size must be a positive integer. The program uses a deterministic two-layer ReLU network with eight hidden units and SGD at a learning rate of 0.05. It reports row, epoch and batch counts plus initial and final mean squared error. Failed training exits with status 1; invalid arguments exit with status 2.

An optional fourth argument selects a zero-based starting row:

```sh
oxid run -- data/sample.oxds 30 4 3
.oxid/bin/streaming-training data/sample.oxds 30 4 3
```

These commands train on rows 3 through 7 of the eight-row sample. The default is row 0. Every epoch and both loss measurements use the selected suffix; reported row and batch counts describe that suffix. A starting row must be smaller than the dataset's row count. The file must support seeking when the start is greater than zero.

The reader validates the header, selected rows and exact file ending. Skipped rows are outside that validation, even when prefetched while reading the header.

## Your dataset

The file starts with this 24-byte header. Every number uses little-endian byte order.

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 8 bytes | ASCII `OXDS0001` |
| 8 | 4 bytes | Unsigned row count |
| 12 | 4 bytes | Unsigned input column count |
| 16 | 4 bytes | Unsigned target column count |
| 20 | 4 bytes | Zero |

After the header, each row contains its input values followed by its target values, all as finite IEEE 754 binary64 numbers. There is no padding or checksum. Missing selected bytes, extra bytes, invalid dimensions, NaN and infinity in selected rows cause an error. Finite data changes require an application-level integrity check if one is needed.

Rows must be between 1 and 2,147,483,647. Input and target column counts must both be positive, with a combined maximum of 268,435,455. Actual batch allocation must fit the execution heap and tensor size limits; these format limits do not guarantee allocation.

For example, export two input columns and one target column from Python:

```python
import struct

rows = [(0.0, 1.0, 0.5), (1.0, 0.0, 3.0)]
with open("training.oxds", "wb") as output:
    output.write(b"OXDS0001" + struct.pack("<IIII", len(rows), 2, 1, 0))
    for row in rows:
        output.write(struct.pack("<ddd", *row))
```

```sh
.oxid/bin/streaming-training training.oxds 30 4
```

Each epoch reads selected rows in file order. Pass a seed to shuffle them for training.

```sh
.oxid/bin/streaming-training training.oxds 30 4 --shuffle 42
```

The seed is an integer from 0 to 4,294,967,295. The trainer reads up to eight batches at a time and shuffles rows within that group. Memory use depends on the batch size and row width. Use the same seed, dataset and batch size to repeat the row order. If you pass a starting row, put it before `--shuffle`.

## Resume training

Add `--checkpoint` with a new filename prefix to save the initial model and each completed epoch. Use `--resume` with the same prefix, dataset, batch size, starting row and shuffle settings. The epoch argument on resume specifies how many additional epochs to run.

```sh
.oxid/bin/streaming-training training.oxds 10 4 --shuffle 42 --checkpoint run
.oxid/bin/streaming-training training.oxds 20 4 --shuffle 42 --resume run
```

Keep both `run.a` and `run.b`. They alternate between saves. If the newest file is incomplete or corrupt, training resumes from the latest valid file. Each save includes the model, completed epoch count, shuffle state and a checksum of the selected rows. Use one trainer per prefix and keep the dataset unchanged. The summary reports cumulative epochs and batches, with initial loss measured at the start of that invocation.

On Linux and macOS, press Ctrl+C or send SIGTERM to stop between training
batches. The program exits with status 130. To continue, run the same
`--resume` command with the number of additional epochs you want. Resume starts
from the most recent valid checkpoint.

## Build on another machine

Create a source archive from the project directory.

```sh
oxid lock --offline
oxid package --frozen --output streaming-training.oxpkg.tar
```

Copy the archive and record the SHA-256 printed by `oxid package`. On the
destination machine, use that checksum to extract and build the project offline.

```sh
oxid package --extract streaming-training.oxpkg.tar --sha256 "$checksum" --output unpacked
cd unpacked/source/streaming-training
oxid test --frozen
oxid build --frozen --profile release
.oxid/bin/streaming-training data/sample.oxds 30 4
```

Set `checksum` to the recorded SHA-256 before running these commands. To resume
an existing run, transfer your dataset and both checkpoint files separately.

## Use the reader in Oxid

Add the [reader package](reader/README.md) and [tensor package](../cpu-training/tensor/README.md) as path dependencies. `reader::open(path, batch_size)` returns a reader; `next(&mut reader)` returns owned input and target tensors for the next batch. The final batch may contain fewer rows.

`reader::open_at(path, batch_size, start_row)` starts at an absolute row without parsing the earlier rows. `position` remains an absolute row number; `remaining` reports the unread suffix. The byte offset uses checked signed 64-bit arithmetic, so a selected row can begin beyond 4 GiB without loading the prefix.

`row_count`, `feature_count`, `target_count`, `position` and `remaining` report dimensions and progress. A reader uses a 4 KiB input buffer plus its current batch. Keeping previously returned batches also keeps their memory allocated.

A failed batch does not advance `position`. It closes the file and leaves the reader poisoned; later calls return `Error::Poisoned`. The final batch checks end-of-file and closes the file before it is returned. Further calls return `Error::End`. Dropping a reader also closes its file.

Run the reader test with `oxid test`.
