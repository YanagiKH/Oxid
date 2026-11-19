# matrix

Owned, dynamically sized `f64` matrices for Oxid. Create a matrix from a slice
or vector, read and update its cells, transpose it, or multiply two matrices.

```toml
[dependencies]
matrix = "../../packages/matrix"
```

`from_slice(rows, columns, &values)` copies values in row order.
`matmul(&left, &right)` creates a new product. Both return
`MatrixResult::Ok(matrix)` or `MatrixResult::Err(error)`.
See the [complete example](../../examples/typed-matrix/src/main.ox):

```sh
cd examples/typed-matrix
oxid run
oxid test
```

The example multiplies a 2 × 3 matrix by a 3 × 2 matrix. The result is
`[[58, 64], [139, 154]]`; the program prints its last cell, `154`.

| Function | Behavior |
| --- | --- |
| `zeros(rows, columns)` | Allocate a matrix filled with positive zero. |
| `from_slice(rows, columns, &[f64])` | Copy an exactly matching number of values. |
| `from_vector(Vector, rows, columns)` | Take ownership without copying; return the original vector on an invalid shape. |
| `into_vector(Matrix)` | Consume the matrix and return its values without copying or allocating. |
| `rows(&Matrix)` / `columns(&Matrix)` | Read the dimensions. |
| `get(&Matrix, row, column)` | Return `Read::Value(number)` or `Read::OutOfBounds`. |
| `set(&mut Matrix, row, column, value)` | Replace a cell; return `false` for an invalid index. |
| `transpose(&Matrix)` | Create a new matrix with exchanged rows and columns. |
| `matmul(&Matrix, &Matrix)` | Multiply compatible matrices without changing either input. |

Import `Read` from the matrix package when matching cell values.
A separate `float_vector` dependency is only needed when transferring vectors.
Dimensions and indices use `i32`. Negative dimensions are invalid; zero
dimensions are allowed. The inner dimensions must match for multiplication.
An empty inner dimension produces positive zero cells. Empty results do not
iterate over their potentially large outer dimension.

`Error` distinguishes `InvalidShape`, `ShapeMismatch`, `CapacityOverflow`,
`AllocationFailed` and `MemoryLimit`. Matrices share the runtime's 16 MiB live
allocation limit with other buffers. Failed operations preserve their inputs.
The private shape always matches the initialized element count.

Construction, indexed copies, ownership transfer and transpose preserve exact
bits, including negative zero and NaN payloads. Multiplication uses `f64`
arithmetic and accumulates each cell in increasing inner-index order, starting
at positive zero, with separate multiplication and addition. This package
currently provides two-dimensional matrices.
