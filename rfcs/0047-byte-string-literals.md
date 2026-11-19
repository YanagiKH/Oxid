# Byte string literals

Status: experimental in `typed-preview`.

`b"Hello, Oxid 👋"` produces a fixed byte array containing UTF-8. The array's
length is the number of decoded bytes, so this example has type `[u8; 16]`.
`b""` has type `[u8; 0]`. Literals accept at most 1024 decoded bytes and use
the existing move-only array ownership, indexing, mutation and borrowing rules.

```rust
fn main() -> i32 {
    let greeting = b"Hello, Oxid 👋";
    return greeting.len();
}
```

The `b` must directly precede the opening quote. The contents may include raw
Unicode text or the escapes `\\`, `\"`, `\n`, `\r`, `\t`, `\0`, `\xNN`
and `\u{H}`. Hexadecimal bytes require exactly two ASCII hexadecimal digits;
Unicode escapes require one through six such digits and a valid Unicode
scalar value. Hexadecimal escapes can construct arbitrary binary data,
including NUL and bytes above 127. Unicode escapes encode their scalar as UTF-8.
Underscores, raw CR/LF, unknown or incomplete escapes, surrogate code points and
values above U+10FFFF are errors. Raw UTF-8 is an Oxid extension to Rust's ASCII
byte string spelling.

Invalid contents produce E0101 at the offending source range. More than 1024
decoded bytes produces E0400. Both are parser errors and occur before native
tools run or existing build outputs change. Exact array type mismatches and
borrow/move errors retain their ordinary diagnostics. Bare quoted strings,
raw strings, character literals, scalar byte literal syntax, implicit copies
and implicit integer-to-byte conversions are outside this feature.

The lexer retains its existing identifier and quoted-token representation.
Parsing validates the complete decoded length without allocating a byte vector;
the AST retains source spans and the checked length. Resolution stores decoded
bytes in an explicitly accounted vector. Type checking produces the existing
fixed-array type. Lowering emits byte constants and the existing array
construction instructions. Interpreter and native execution share those owned
array operations; this adds no runtime buffer or machine-frame fields.
Formatting keeps the literal's contents and escape spellings exactly.

Public tests cover empty and maximum arrays, raw UTF-8, Unicode boundaries,
all 256 byte values, moves, mutation, nested owners, stdout and malformed input.
Native checks execute both optimization profiles. Allocation/work failure
controls check that decoding storage is charged and released. Retained
compatibility checks cover prior behavior; new literal semantics require their
own tests and do not establish stable-language release readiness.
