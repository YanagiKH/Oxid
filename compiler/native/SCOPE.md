# Standalone scalar compiler

This experimental compiler reads an Oxid source file and emits LLVM. Its lexer,
parser, name/type checks, control-flow lowering and emitter are written in Oxid.
The native compiler can rebuild its Linux or macOS executable from source with Clang.

Build and use it from the repository root:

```sh
(cd compiler/native && oxid build --profile=release)
python3 compiler/native/build.py input.ox output --compiler compiler/native/.oxid/bin/oxid-scalar-compiler --clang /path/to/clang
```

To rebuild the compiler, pass `compiler/native/src/main.ox` as the input
and add `--profile release --entry-mode process`. Use the resulting executable
as `--compiler` to build another program or repeat the rebuild.
When using an unpacked compiler kit, use `compiler/native/bin/oxid-scalar-compiler`
for `--compiler`. The kit includes the Oxid source and C runtime. Rebuilding
requires Python, Clang 19.1.7 and LLD 19.1.7. On Linux, install the system
development libraries. On macOS, install the Command Line Tools or Xcode SDK.
Set `OXID_LLVM_BIN` to the directory containing `clang` and `ld64.lld`.

The project uses process entry mode: a successful compiler exits 0 and writes
only one complete LLVM module to stdout, with no printed main return value.
The driver rejects failed status, stderr diagnostics, missing framing, partial
output or any bytes after the module footer. It never strips a printed `0` or
other lines. The driver selects the host target on Linux and macOS with x86-64 or ARM64.
LLVM/Clang and the scalar C I/O adapter are explicit dependencies;
they do not parse or check Oxid source. Existing output paths are refused.

Supported input uses ASCII identifiers, i32/i64/u8/u32/bool/char/unit, named functions
with typed parameters/results, initialized let bindings, optional type
annotations, mut/assignment, lexical blocks, calls, if/else, while, for, break,
continue, explicit returns and short-circuit boolean operators. Active names
cannot shadow bindings or functions. Integer arithmetic is checked; division
and remainder reject zero and signed MIN/-1 before the LLVM operation. Use
decimal literals such as `255u8` and `9007199254740993i64`. Convert local
bindings with `.to_i64()`, `.to_i32_checked()` or `.to_u8_checked()`.
Use `&`, `^` and `|` on two integers of the same type, and `~` to flip all bits.
Use `<<` or `>>` with an integer shift count from zero up to the value's bit
width, excluding the width itself. Right shifts preserve the sign of signed
values. Left shifts discard bits that move beyond the width.
A result-mode `main` has no arguments and prints its scalar result, then exits 0.
Use `build.py --entry-mode=process` with `fn main() -> i32` to return an exit
status without printing it. Valid exit statuses range from 0 to 255.
Import `std::env::arg_count` to count the executable and the arguments
passed to it. The executable is included, so the count starts at 1.
Use `std::env::arg_bytes(index, &mut bytes)` to copy an argument as raw
bytes. Index 0 is the executable. Match `std::env::ArgStatus` to get the
copied byte count or the required size when the destination is too small.
In process mode, `std::io::read_some(&mut bytes)` reads from stdin.
`write_some(&bytes)` writes to stdout, and `write_stderr(&bytes)` writes to
stderr. Match `ReadSomeStatus` or `WriteSomeStatus` for the transferred byte
count, end of input or an operating system error.
Use `alloc::bytes::new()` to create an empty `ByteBuf`. Use `try_resize`,
`try_push` or `try_extend` to add bytes, and match `GrowStatus` to check
whether the operation succeeded. Borrow the buffer as `&[u8]` or
`&mut [u8]` to read and write individual bytes by index. Buffers can
move between functions and free their storage when their owner goes out of scope.
Store a buffer in a struct field or enum variant to give that value ownership
of the bytes. Use `take(&mut field)` to move a buffer out and leave it empty.
Create a vector with `alloc::vec::new::<i32>()`. Vectors accept
`i32`, `u8`, `i64`, `u32`, `bool` and `char` elements.
Use `try_push` to add an element, `values[index]` to read or replace one,
and `&values[start..end]` to borrow a slice. Use `values.len()` to get the
current length. The same operations work on a vector stored in a struct.
Vectors of `#[derive(Copy)]` structs support indexing and borrowing slices.
Use `try_extend` to copy elements from a slice of the same type.
Use `std::fs::open_read(&path)` with a byte path and match `OpenResult`.
Call `std::fs::read_some(&mut file, &mut bytes)` to read data into a slice.
The file closes when its owner goes out of scope. Use `std::fs::close`
to close it earlier and check the result.
Use `std::fs::create` to create or replace a file, or `open_append` to
add to an existing file. Check how many bytes `write_some` wrote. Use
`sync_all` to flush changes to storage and `seek_absolute` to change the byte offset.
Use `for index in start..end { ... }` to iterate over matching integer bounds.
The bounds evaluate once from left to right, and the end is excluded. The
index is immutable and available inside the loop body.
Use `'雪'` or `'\u{1f980}'` for a Unicode character. A `char` can be passed,
returned, stored in a record or compared by Unicode scalar value. A `char`
result prints UTF-8. Comments and byte string literals also accept UTF-8.
Use `number.to_char_checked()` to reject invalid Unicode scalar values and
`character.to_u32()` to return a character's number. Use a `u32` suffix for
unsigned integers such as `4294967295u32`, and `.to_u32_checked()` or
`.to_i64_checked()` for checked conversions between integer types.

Use `[i32; 3]` for a fixed array type and `[10, 20, 30]` for a value.
An array holds up to 1024 values of the same type. Use `i32`, `i64`, `u8`,
`u32`, `bool` or `char`. Passing or assigning a whole array moves it. Use a new value to reinitialize
a mutable binding after a move. A loop can move arrays and owned structs declared inside its body.
Use `data.len()` for the length and `data[index]` to read an element. For a
`mut` binding, use `data[index] = value` to replace an element. Reads and
writes check the index. Use `b"abc"` for the byte array `[97u8, 98u8, 99u8]`
and `b""` for an empty byte array. Byte strings support byte and Unicode escapes.

Use a shared slice parameter to read a fixed array during a call.

```rust
fn first(values: &[i32]) -> i32 {
    return values[0];
}

fn main() -> i32 {
    let numbers = [42, 0];
    return first(&numbers);
}
```

Inside the function, check the length with `values.len()` and read by index.
Pass `&numbers[start..end]` to borrow a range. The start is included and the
end is excluded. Pass `&*values` or `&*values[start..end]` to share a slice with
another function.

Use `&mut [i32]` for a parameter that can replace elements. Pass a mutable
array with `&mut numbers` or pass a range with `&mut numbers[start..end]`.
Inside the function, assign through the slice with `values[index] = value`.
Pass `&mut *values` to let another function write to it. During a mutable
borrow, the owning binding is reserved until the call returns.

Declare `#[derive(Copy)] struct Point { x: i32, y: i32 }` to group scalar
fields. Construct it with `Point { y: 2, x: 40 }` and read a field with
`point.x`. Use a `mut` binding to replace a field. Copies and function
arguments hold independent values. These structs are private to their
module by default and have up to 64 fields of type `i32`, `i64`, `u8`, `u32`,
`bool` or `char`. Declare `pub struct` to use a record in another module.
Refer to it by its `crate::module::Type` path or import it with `use`.
Declare fields with `pub` to allow other modules to read, write or initialize them.

Declare `struct Row { id: i32, bytes: [u8; 2] }` to group scalars and fixed
arrays in an owned value. Passing or assigning the value moves it. Use
`let Row { id: row_id, bytes: data } = row;` to take all its fields. To replace
an array field, assign a new value to the whole mutable struct.

Struct fields can use types declared elsewhere in the project. For example, use
`struct Batch { item: Row }`, then `let Batch { item: row } = batch;` to take
the nested value. Construction accepts up to eight levels of struct nesting.
All nested fields of a `Copy` struct must also be `Copy`.
Read a nested scalar with `batch.item.id`. A mutable binding or an `&mut`
parameter can assign to the same field path.
Use `struct Empty {}` for a type with zero fields. Construct it with `Empty {}`
and consume an owned value with `let Empty {} = value;`.

Use `row: &Row` to read scalar fields during a call. Use `row: &mut Row` to
change those fields. Pass `&row` or `&mut row` from the owner, or pass `&*row`
and `&mut *row` from a borrowed parameter. The same struct can fill multiple
shared parameters in one call. An exclusive borrow reserves the owner until
the call returns.

Pass a fixed array field as a slice with `&row.bytes` or a range with
`&mut row.bytes[start..end]`. A borrowed record can pass its field with
`&*row.bytes` or `&mut *row.bytes`. Separate fields can be borrowed mutably
in the same call.

Borrow a nested struct with `&row.section.item` or `&mut row.section.item`.
Separate nested fields can be borrowed mutably together. A borrow of an
entire struct overlaps any borrow of its fields.

Declare `enum Value { Empty, Count(i64) }` and construct `Value::Count(42i64)`.
A variant can carry one scalar, fixed array or struct.
Match a binding and give one arm for each variant.

```rust
let value = Value::Count(42i64);
match value {
    Value::Empty => { return 0i64; },
    Value::Count(count) => { return count; },
}
```

Passing, assigning or matching an enum moves it.
Use `pub enum` to share the type with another module. Construct and match
variants through an imported type name or a path such as
`crate::messages::Value::Count`.

Calls can be recursive. Compiled programs allow up to 128 active Oxid function
calls including `main`. The next call exits with status 1 and reports
`call depth limit exceeded` on standard error. Non-unit functions must return on
all paths. Loop conditions are checked conservatively. External package imports,
nested enums, other reference forms, user-defined generics, text strings, nested arrays
and other integer types are unsupported.

Declare `mod math;` in the entry file to load `math.ox`. Inside `math.ox`,
`pub mod stats;` loads `math/stats.ox` and exposes it outside `math`.
Use `pub fn` to expose a function and call it with
`crate::math::stats::function(...)`. Import it with
`use crate::math::stats::function;` or rename it with `as name` for unqualified calls.
Source files share the input and function limits. The compiler accepts up to
128 source files including the entry file and up to 32 levels of modules.

Input is limited to 512 KiB, 131072 tokens, 512 functions and 16 parameters per
function. A project can declare up to 128 structs and enums and use up to 16384
call sites. Each function has a 32 KiB local storage budget. Expression/block/comment nesting
is limited to 64 and LLVM output to 8 MiB. Each function also has checked budgets
of 4096 active bindings, 131072 temporary values and 65536 labels.
Compiler status 64 means usage, 65 rejected
source, 66 input failure, 68 resource refusal and 74 output/I/O failure. Rejected
source writes the file, line and byte column to standard error and emits no LLVM.
The build command displays the same location.
