# Owned UTF-8 Text source prototype

This ordinary two-module `typed-preview` project builds on the experimental
`alloc::bytes::ByteBuf` contract in RFC 0032 (reviewed source baseline
`156714f67b2e11b9f00c809acf6b1b9484d04dcb`). It adds no compiler operation,
runtime code, allocator, or native UTF-8 kernel. It is not canonical stdlib,
a stable `String` API, generic container support, or completion of R2.

`src/text.ox` exposes:

- `Text`, whose sole `ByteBuf` field is private.
- `new_text()`, `byte_len(&Text)`, and `scalar_count(&Text)`.
- `validate(&[u8]) -> Validation`: `Valid` or `Invalid(byte_offset)`.
- `from_bytes(ByteBuf) -> FromBytes`: `Ok(Text)` or `Invalid(ByteBuf)`.
- `append_utf8(&mut Text, &[u8]) -> AppendStatus`: `Ok`,
  `InvalidInput(byte_offset)`, `CapacityOverflow`, `AllocationFailed`, or
  `MemoryLimit`.
- `truncate(&mut Text, byte_length) -> bool`.

Validation is strict and complete-input: ASCII (including NUL), BOM and Unicode
noncharacters are allowed; overlong sequences, surrogates, values above
U+10FFFF, stray continuations, invalid leads and incomplete tails are refused.
An error offset identifies the start of the first ill-formed scalar sequence,
not the later offending continuation byte. All offsets and lengths are `i32`.

Successful conversion moves the original buffer into Text. Invalid conversion
returns that original owner without replacing or modifying it; call `validate`
first if its error offset is needed. Append validates before the single
`try_extend`, and forwards all recoverable growth refusals. Thus invalid input
and returned growth failures leave the old owner, bytes, length and capacity
unchanged under RFC 0032. Terminal execution-budget failures remain terminal;
they are not append statuses. Truncation refuses negative lengths, lengths
past the end, and positions inside a scalar, leaving the text unchanged.
Zero and the current length are valid boundaries.

There is no mutable byte escape, `into_bytes`, partial field move, lossy
conversion, normalization, grapheme segmentation, text indexing, streaming,
or implicit clone. Scalar count counts Unicode scalar values, not graphemes;
for example `e` plus U+0301 counts as two scalars. Validation and scalar count
are linear, allocation-free source loops. Boundary checking is constant-time.

## Focused acceptance

The partitioned client passed source checking (32 functions), interpreter
execution, LLVM compilation, and standalone native execution using the
compatible compiler source tree
`f6d19c23f5b69936245d4f4f8991100e92a86383`. The interpreter
and standalone native runs printed exactly `42\n`, exited 0, and left stderr
empty. The checked client source SHA-256 is
`4d9078df8d45dc36f64a4f71cb71360be28c702582930051bb38c9f278377e7d`.

The original single-function client exceeded the existing native per-function
slot ceiling and was rejected with E0700. Splitting its fixed checks into
small client functions resolved that rejection. The library, compiler limits,
and expected assertions were unchanged.

To reproduce the source and interpreter checks from a compatible checkout:

```sh
oxid check --manifest-path examples/projects/owned_utf8_text/oxid.toml
oxid run --manifest-path examples/projects/owned_utf8_text/oxid.toml --engine interpreter --entry-mode result
```

With the existing supported native toolchain configured, reproduce the
standalone native route using:

```sh
oxid compile examples/projects/owned_utf8_text/src/main.ox --edition=typed-preview --entry-mode=result --backend=llvm --target=x86_64-unknown-linux-gnu --output=/tmp/owned-utf8-text
/tmp/owned-utf8-text
```

The focused native result is for that standalone executable; no separate
manifest-native-run result is claimed.

The client checks valid UTF-8 range edges, malformed/truncated cases and byte
offsets, invalid conversion's returned bytes, mixed-width conversion, NUL and
combining-scalar append, invalid/empty append, and invalid/valid truncation.
Negative results identify failed assertions. These checks do not force
allocation failures or establish cleanup acceptance; those require the
existing ByteBuf fault and resource checks. This is focused prototype
acceptance, not general String or R2 completion.

Fixture cross-checking against a host decoder is an independent oracle only.
Separate Rust/C UTF-8 kernels are not called by this project and do not
establish its interpreter/native result.
