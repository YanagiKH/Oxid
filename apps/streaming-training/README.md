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

Skipping rows does not restore model weights or optimizer state: each command initializes a new model. It validates the header, selected rows and exact file ending. **Skipped rows are not validated**, even if their bytes were prefetched while reading the header. A corrupt skipped value will therefore not reject an otherwise valid selected suffix.

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

Each epoch reads selected rows in file order, seeking directly to the first selected row. Keep the dataset unchanged during the run. This example does not shuffle rows, save a trained model or resume an interrupted run. For resumable training with an in-memory dataset, see [minibatch training](../minibatch-training/README.md).

## Use the reader in Oxid

Add the [reader package](reader/README.md) and [tensor package](../cpu-training/tensor/README.md) as path dependencies. `reader::open(path, batch_size)` returns a reader; `next(&mut reader)` returns owned input and target tensors for the next batch. The final batch may contain fewer rows.

`reader::open_at(path, batch_size, start_row)` starts at an absolute row without parsing the earlier rows. `position` remains an absolute row number; `remaining` reports the unread suffix. The byte offset uses checked signed 64-bit arithmetic, so a selected row can begin beyond 4 GiB without loading the prefix.

`row_count`, `feature_count`, `target_count`, `position` and `remaining` report dimensions and progress. A reader uses a 4 KiB input buffer plus its current batch. Keeping previously returned batches also keeps their memory allocated.

A failed batch does not advance `position`. It closes the file and leaves the reader poisoned; later calls return `Error::Poisoned`. The final batch checks end-of-file and closes the file before it is returned. Further calls return `Error::End`. Dropping a reader also closes its file.

Run the reader test with `oxid test`.
