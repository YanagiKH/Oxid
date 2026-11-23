# Owned vectors

Use `try_push` to store a value and recover it if allocation fails. `pop`
returns the last value, and `detach` takes the entire vector.

Call `try_append(&mut destination, &mut source, &mut buffer)` to move all
values from `source` to the end of `destination` in order. The buffer keeps
its contents and can be reused. If reserving storage fails, `source` and
`destination` keep their contents. The buffer may retain additional capacity.
