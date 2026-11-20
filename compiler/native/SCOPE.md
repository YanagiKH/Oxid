# Standalone scalar compiler

This experimental compiler reads an Oxid source file and emits LLVM. Its lexer,
parser, name/type checks, control-flow lowering and emitter are written in Oxid.
It does not yet compile its own implementation or satisfy the self-hosting gate.

Build and use it from the repository root:

```sh
(cd compiler/native && oxid build --profile=release)
python3 compiler/native/build.py input.ox output --compiler compiler/native/.oxid/bin/oxid-scalar-compiler --clang /path/to/clang
```

The project uses process entry mode: a successful compiler exits 0 and writes
only one complete LLVM module to stdout, with no printed main return value.
The driver rejects failed status, stderr diagnostics, missing framing, partial
output or any bytes after the module footer. It never strips a printed `0` or
other lines. LLVM/Clang and the scalar C I/O adapter are explicit dependencies;
they do not parse or check Oxid source. Existing output paths are refused.

Supported input uses ASCII identifiers/comments, i32/i64/u8/bool/unit, named functions
with typed parameters/results, initialized let bindings, optional type
annotations, mut/assignment, lexical blocks, calls, if/else, while, break,
continue, explicit returns and short-circuit boolean operators. Active names
cannot shadow bindings or functions. Integer arithmetic is checked; division
and remainder reject zero and signed MIN/-1 before the LLVM operation. Use
decimal literals such as `255u8` and `9007199254740993i64`. Convert local
bindings with `.to_i64()`, `.to_i32_checked()` or `.to_u8_checked()`.
A result-mode `main` has no arguments and prints its scalar result, then exits 0.

Use `b"..."` for fixed byte arrays such as `let data: [u8; 3] = b"abc";`.
Literals support byte and Unicode escapes. Call `data.len()` for the length
and read a byte with `data[index]`. Indices are checked before reading.

Calls can be recursive. Compiled programs allow up to 128 active Oxid function
calls including `main`. The next call exits with status 1 and reports
`call depth limit exceeded` on standard error. Non-unit functions must return on
all paths. Loop conditions are checked conservatively. External package imports,
user records and enums, references, generics, text strings, non-byte arrays,
I/O in compiled input and other integer types are unsupported. The compiler
itself uses ByteBuf, File and Copy vectors.

Declare `mod math;` in the entry file to load `math.ox`. Inside `math.ox`,
`pub mod stats;` loads `math/stats.ox` and exposes it outside `math`.
Use `pub fn` to expose a function and call it with
`crate::math::stats::function(...)`. Import it with
`use crate::math::stats::function;` or rename it with `as name` for unqualified calls.
Source files share the input and function limits. The compiler accepts up to
128 source files including the entry file and up to 32 levels of modules.

Input is limited to 65536 bytes, 32768 tokens, 128 functions, 16 parameters/function,
4096 call sites and 4096 stack slots/function. Expression/block/comment nesting
is limited to 64 and LLVM output to 8 MiB. Each function also has checked budgets
of 4096 active bindings, 131072 temporary values and 65536 labels.
Compiler status 64 means usage, 65 rejected
source, 66 input failure, 68 resource refusal and 74 output/I/O failure. Rejected
source emits no LLVM, including when earlier functions were already buffered.
