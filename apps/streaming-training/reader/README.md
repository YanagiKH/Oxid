# Tensor dataset reader

Read a binary dataset one batch at a time using owned files and tensors.

Add local dependencies to your project:

```toml
[dependencies]
reader = "../Oxid/apps/streaming-training/reader"
tensor = "../Oxid/apps/cpu-training/tensor"
```

Adjust the paths to your checkout. `reader::open(path, batch_size)` returns `reader::Result<reader::Reader>`. Call `reader::next(&mut reader)` to receive a `tensor::data::Batch`; its input and target tensors belong to the caller.

`row_count`, `feature_count`, `target_count`, `position` and `remaining` describe dimensions and progress. A failed batch preserves its position, closes the file and poisons later reads. The final batch verifies EOF and closes the file; subsequent reads return `Error::End`. Dropping a reader releases its file and buffer.

The reader uses a fixed 4 KiB buffer plus the returned batch. Keeping older batches keeps their memory allocated. It reads rows sequentially and validates dimensions, finite values and the exact file ending. See the [training example](../README.md) for the binary format and commands.
