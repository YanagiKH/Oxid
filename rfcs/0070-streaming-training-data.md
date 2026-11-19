# Sequential training datasets

Status: experimental

The reader package can consume a dataset in batches without holding the full dataset in memory. The application in `apps/streaming-training` exercises this path with the existing model and SGD implementation.

## Reader

`reader::open(path, batch_size)` opens an owned file and validates its header. `next(&mut reader)` returns an owned `Batch` containing the input and target matrices. Rows remain in file order, and the final batch may be smaller than the requested size. `position` counts rows returned successfully; `remaining` counts rows not yet returned.

A reader owns a 4 KiB byte buffer. Each call allocates only its input and target matrices. Previous batches remain owned by their caller. Allocation, shape and execution limits still apply, so the header's numerical bounds do not promise that a requested batch will fit.

Reads may return fewer bytes than requested. The reader fills each destination slice across as many reads as needed and retries interrupted reads. Unexpected EOF, other I/O errors, invalid dimensions, trailing data and non-finite values produce errors. A failed batch does not advance progress or return partial tensors. It closes the file and poisons the reader. The final successful batch verifies EOF and closes the file before publication; subsequent calls return `End`. Dropping the reader releases the file and scratch storage.

## Wire format

The 24-byte header is ASCII `OXDS0001`, followed by four little-endian u32 values: row count, input column count, target column count and reserved zero. Rows must be in 1..=2,147,483,647. Both column counts must be positive and their sum at most 268,435,455. Payload rows contain finite little-endian IEEE 754 binary64 input values followed by target values, with no padding. The payload ends exactly after the declared final row.

The format provides structural validation, not a checksum or authenticated integrity. A finite payload change can remain a valid dataset. Applications needing integrity must supply it separately.

## Application

The training command accepts a dataset path, epoch count and batch size. It creates a deterministic two-layer ReLU model with eight hidden units and performs SGD at a learning rate of 0.05. Evaluation and training reopen the file for each pass. The dataset must stay unchanged during the run. The command does not provide shuffle, persistence or continuation; those remain separate from existing in-memory resumable sessions.

## Verification

The verification controller executes the public reader test, an independent binary corpus and analytic SGD equations. It checks full and partial batches, buffer boundaries, partial FIFO writes, malformed dimensions, truncated and trailing payloads, non-finite values, I/O errors, heap refusal, poison and end states. Native cases read a dataset larger than the configured heap and observe closed descriptors while the reader remains live. Native dev and release programs run with an empty environment after the original build source directory is moved to a retained location.

The reader is a separate package and uses only public tensor operations; existing tensor source is unchanged. The compiler, ownership checker, runtime and backend are unchanged. This library adds no ABI or memory-safety authority. The controller runs on Linux x86_64 and ARM64 CI hosts; a local run qualifies only its recorded host.
