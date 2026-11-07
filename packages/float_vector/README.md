# float-vector

A growable, owned collection of `f64` numbers. Add a local dependency:

```toml
[dependencies]
vector = "../../packages/float_vector"
```

Create a vector with `new()`, append with `try_push(&mut values, number)`, and
read with `get(&values, index)`. `Read::Value(number)` contains the number;
`Read::OutOfBounds` means the index is negative or outside the vector.
See the [mean example](../../examples/typed-float-vector/src/main.ox) for a
complete program:

```sh
cd examples/typed-float-vector
oxid run
oxid test
```

The example prints `4`. All operations preserve the exact floating-point bits,
including negative zero and NaN payloads.

| Function | Behavior |
| --- | --- |
| `new()` | Create an empty vector without allocating. |
| `len(&Vector)` | Return the number of initialized elements. |
| `capacity(&Vector)` | Return how many elements fit without growing. |
| `try_reserve(&mut Vector, additional)` | Reserve space for additional elements without changing the length. |
| `try_push(&mut Vector, value)` | Append one number. |
| `try_extend(&mut Vector, &Vector)` | Append a copy of another vector. |
| `get(&Vector, index)` | Read one number or return `Read::OutOfBounds`. |
| `set(&mut Vector, index, value)` | Replace a number; return `false` for an invalid index. |
| `truncate(&mut Vector, length)` | Shorten the vector; return `false` for a negative or larger length. |
| `take(&mut Vector)` | Transfer the allocation to a new vector, leaving the original empty and reusable. |
| `into_bytes(Vector)` | Consume a vector and return its bytes without copying. |
| `from_bytes(ByteBuf)` | Take ownership of encoded numbers, or return `FromBytes::InvalidLength` with the original buffer. |

Growth returns `alloc::bytes::GrowStatus`: `Ok`, `CapacityOverflow`,
`AllocationFailed` or `MemoryLimit`. Failure preserves the vector's values,
length and capacity. Vectors share the runtime's 16 MiB live allocation limit
with other buffers. Indices, lengths and additional capacities use `i32`;
each element needs eight bytes. This library currently provides `f64` elements.

The byte representation is a sequence of eight-byte, little-endian IEEE 754
binary64 values with no header. Its length must be divisible by eight. All
bit patterns are valid, including infinities and NaNs. Byte transfer and `take`
neither copy nor allocate. The backing buffer stays private to keep complete
elements intact.
