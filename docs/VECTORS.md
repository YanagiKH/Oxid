# Growable vectors

Use `alloc::vec::Vec<T>` when the number of values changes at runtime. Create
an empty vector with `new::<T>()`. Scalar and Copy elements support indexing;
owned records move into and out of the vector.

```oxid
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::try_push;

fn main() -> i32 {
    let mut values = new::<i32>();
    let added = try_push(&mut values, 42);
    match added {
        GrowStatus::Ok => { return values[0]; },
        GrowStatus::CapacityOverflow => { return -1; },
        GrowStatus::AllocationFailed => { return -1; },
        GrowStatus::MemoryLimit => { return -1; },
    }
}
```

The [complete example](../examples/typed-vectors) also appends an array. Run it
with a compiler built from this source tree:

```sh
cd examples/typed-vectors
oxid run
oxid test
oxid build --profile release
```

`oxid run` prints `42`. The published 0.9 binaries do not include this API.

## Choose an element type

Vectors support `bool`, `()`, `char`, `f32`, `f64`, `i8`, `i16`, `i32`, `i64`,
`i128`, `isize`, `u8`, `u16`, `u32`, `u64`, `u128` and `usize`. Each vector has
one element type. Floating-point values preserve their bits, including negative
zero and NaN payloads.

`Vec<T>` owns its allocation. Moving it transfers ownership; leaving its scope
releases its storage. A vector is not `Copy`, including when `T` is `Copy`.
Vectors can be fields and enum payloads. A finite record declared with
`#[derive(Copy)]` can also be an element. Its fields must be scalar values or
other checked Copy records. Finite non-Copy records can contain buffers, files,
scalar or Copy vectors, and finite enum payloads; use the owned-record operations
below. Direct enum elements, nested vectors of owned records, and stored or
returned references are unavailable.

## Store records

Declare a record Copy, then use the same vector functions:

```oxid
#[derive(Copy)]
struct Point { x: i32, y: i32 }

fn first(points: &[Point]) -> Point { return points[0]; }

fn main() -> i32 {
    let point = Point { x: 20, y: 1 };
    let mut points = new::<Point>();
    try_push(&mut points, point);
    let snapshot = first(&points);
    points[0] = Point { x: 22, y: 1 };
    let changed = first(&points);
    return snapshot.x + changed.x;
}
```

Import `new` and `try_push` as in the first example. This program prints `42`.
An indexed read produces an independent value. `try_push`, `try_resize` and
whole-element assignment also leave their Copy argument available. Distinct
record declarations remain distinct types even when their fields match.

Use `&[Point]`, `&mut [Point]`, or a generic `&[T]` with `T: Copy` to borrow
record elements. Record slices come from vectors; fixed arrays still require
scalar elements. References to an individual element or field are unavailable.
The [complete record-vector example](../examples/copy-record-vectors) covers
all nine operations and generic slice helpers.

## Store messages with owned resources

Use a non-Copy record when each element owns a buffer or file. Reserve space
with `try_reserve`, check for `GrowStatus::Ok`, then move a record into the
vector with `push_reserved(&mut values, message)`. This insertion does not
allocate and returns `()`; inserting without spare capacity stops execution.
The moved message is no longer available to the caller.

Check `len(&values)` before calling `pop_last(&mut values)`. It removes and
returns the last record, transferring ownership to the caller. Pop the record
before accessing its fields: indexing, slices, and references to individual
owned-record elements are unavailable.

These vectors provide eight functions: `new`, `len`, `capacity`, `try_reserve`,
`push_reserved`, `pop_last`, `truncate`, and `take`. `truncate` releases removed
records and their active resources. `take` transfers the allocation and leaves
the source with zero length and capacity. Leaving scope releases the remaining
records and storage. `try_push`, `try_extend`, and `try_resize` require Copy
elements and do not accept these records.

The [message example](../examples/owned-record-vectors) checks allocation
results, transfers a batch, and removes its message:

```sh
cd examples/owned-record-vectors
oxid run
oxid test
oxid build --profile release
```

`oxid run` prints `42`. Build the compiler from this source tree to use this API.

## Add, resize and move values

For scalar and Copy elements, import each function from `alloc::vec`:

| Function | Effect |
| --- | --- |
| `new::<T>()` | Create an empty vector without allocating. |
| `len(&values)` | Return the number of initialized elements. |
| `capacity(&values)` | Return how many elements fit before growth. |
| `try_reserve(&mut values, additional)` | Reserve room for additional elements; keep the length. |
| `try_push(&mut values, value)` | Append one element. |
| `try_extend(&mut values, &source)` | Append the values of a matching array, slice or vector. |
| `try_resize(&mut values, length, fill)` | Grow with copies of `fill`, or shorten the vector. |
| `truncate(&mut values, length)` | Shorten the vector; a larger length leaves it unchanged. |
| `take(&mut values)` | Transfer storage into a new vector, leaving the original empty. |

Lengths, indices and capacity arguments use nonnegative `i32` values and count
elements. The four `try_` operations return `GrowStatus::Ok`,
`CapacityOverflow`, `AllocationFailed` or `MemoryLimit`. A failed growth leaves
the values, length and capacity unchanged. Vectors share the execution policy's
heap limit with byte buffers and files. The bounded policy also limits execution
work; the hosted policy is intended for ordinary native programs.

## Borrow a slice

Pass `&values` or `&values[start..end]` to a matching `&[T]` parameter. Use
`&mut values` for an exclusive `&mut [T]` parameter. Ranges exclude `end`, and
indexing or ranges outside the initialized elements stop execution with a bounds
error.

```oxid
fn second<T: Copy>(values: &[T]) -> T {
    return values[1];
}
```

For scalar elements, the same slice functions also accept fixed arrays. A shared borrow prevents changes
to its owner; an exclusive borrow prevents other access until the call ends.
Forward a slice parameter with `&*values` or `&*values[start..end]`.

`Vec<u8>` and `ByteBuf` are distinct owned types. Either can provide a `&[u8]`
or `&mut [u8]` for byte I/O, subject to the same borrowing rules. Other element
types cannot be passed as raw byte slices.
