# Tensor dataset reader

Read a binary dataset one batch at a time using owned files and tensors.

Add local dependencies to your project:

```toml
[dependencies]
reader = "../Oxid/apps/streaming-training/reader"
tensor = "../Oxid/apps/cpu-training/tensor"
```

Adjust the paths to your checkout. `reader::open(path, batch_size)` returns `reader::Result<reader::Reader>`. Call `reader::next(&mut reader)` to receive a `tensor::data::Batch`; its input and target tensors belong to the caller.

Use `reader::open_at(path, batch_size, start_row)` to start at a zero-based row. It requires `0 <= start_row < row_count`; a negative or past-end start returns `Error::InvalidStartRow`. Start 0 has the same sequential behavior as `open`, including support for nonseekable inputs. A positive start requires a seekable file and returns `Error::Io(code)` if the OS refuses the seek. The reader closes that file before returning the error.

After a successful seek, any header prefetch is discarded and `position` becomes `start_row`. `row_count` is still the full header count and `remaining` is `row_count - position`. Only the header, selected rows and exact file ending are validated. Skipped rows are not decoded or checked for finite values. Keep the dataset unchanged while using a reader.

`row_count`, `feature_count`, `target_count`, `position` and `remaining` describe dimensions and progress. A failed batch preserves its position, closes the file and poisons later reads. The final batch verifies EOF and closes the file; subsequent reads return `Error::End`. Dropping a reader releases its file and buffer.

The reader uses a fixed 4 KiB buffer plus the returned batch. Keeping older batches keeps their memory allocated. It computes the absolute offset as `24 + start_row * (features + targets) * 8` with checked `i64` arithmetic, then reads selected rows sequentially. See the [training example](../README.md) for the binary format and commands.
