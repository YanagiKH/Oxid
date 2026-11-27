# Owned vectors

Use `try_push` to store a value and recover it if allocation fails. `pop`
returns the last value, and `detach` takes the entire vector.

Call `try_append(&mut destination, &mut source, &mut buffer)` to move all
values from `source` to the end of `destination` in order. The buffer keeps
its contents and can be reused. If reserving storage fails, `source` and
`destination` keep their contents. The buffer may retain additional capacity.

For scalars and `#[derive(Copy)]` records, use `pop_copy(&mut values)` or
`try_append_copy(&mut destination, &mut source)`. A pop returns
`PopResult::Empty` or `PopResult::Value(value)`. A successful append empties
the source and keeps its capacity for reuse. A failed append leaves both
vectors unchanged.
