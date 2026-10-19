# Owned UTF-8 Text source prototype

This ordinary `typed-preview` path-dependency consumer builds on the experimental
`alloc::bytes::ByteBuf` contract in RFC 0032 (reviewed source baseline
`156714f67b2e11b9f00c809acf6b1b9484d04dcb`). It adds no compiler operation,
runtime code, allocator, or native UTF-8 kernel. It is not canonical stdlib,
a stable `String` API, generic container support, or completion of R2.

The single implementation is in [`packages/utf8_text`](../../../packages/utf8_text).
The direct dependency alias `text` exposes:

- `Text`, whose sole `ByteBuf` field is private.
- `new_text()`, `byte_len(&Text)`, and `scalar_count(&Text)`.
- `validate(&[u8]) -> Validation`: `Valid` or `Invalid(byte_offset)`.
- `from_bytes(ByteBuf) -> FromBytes`: `Ok(Text)` or `Invalid(ByteBuf)`.
- `append_utf8(&mut Text, &[u8]) -> AppendStatus`: `Ok`,
  `InvalidInput(byte_offset)`, `CapacityOverflow`, `AllocationFailed`, or
  `MemoryLimit`.
- `append_scalar(&mut Text, i32) -> ScalarAppendStatus`: `Ok`,
  `InvalidScalar(original_value)`, `CapacityOverflow`, `AllocationFailed`, or
  `MemoryLimit`. This separate enum leaves `AppendStatus::InvalidInput`'s
  byte-offset meaning unchanged.
- `equals_utf8(&Text, &[u8]) -> bool`: exact byte equality with an input slice;
  no allocation, reference escape, or mutation. Invalid UTF-8 cannot equal Text.
- `truncate(&mut Text, byte_length) -> bool`.
- `append_to_bytes(&Text, &mut ByteBuf) -> GrowStatus`: append exact initialized
  bytes to an existing destination, preserving its prefix and the source Text.
  Empty Text is a no-op; recoverable refusal preserves destination state.
  See the package README for copying, allocation and peak-live-memory costs.

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

Scalar append rejects negative values, U+D800..U+DFFF, and values above
U+10FFFF before touching Text. It encodes one accepted value into a four-byte
initialized stack array, then submits only its one-to-four-byte prefix to a
single checked `try_extend`. NUL and noncharacters are accepted. All integer
encoding arithmetic is bounded after the scalar check; `to_u8_checked` is
used only for values in 0..255. Returned growth failures preserve the original
Text under the same ByteBuf contract as `append_utf8`. No allocation-failure
injection is claimed by this client. Equality and scalar count are linear;
scalar encoding itself has bounded constant work (excluding buffer growth).

There is no mutable byte escape, `into_bytes`, partial field move, lossy
conversion, normalization, grapheme segmentation, text indexing, streaming,
or implicit clone. Scalar count counts Unicode scalar values, not graphemes;
for example `e` plus U+0301 counts as two scalars. Validation and scalar count
are linear, allocation-free source loops. Boundary checking is constant-time.

## Package migration validation status

The package migration is not yet executed with Oxid in this preparation
environment. All original checks -1..-57 remain unchanged. New partitioned
checks -58..-70 cover export to empty and prefixed ByteBuf destinations, exact
mixed-width/NUL bytes, unchanged source, and empty Text export. The regression
copies package and consumer into a fresh repository-shaped directory, preserving
relative manifest paths. Private-storage and moved-owner negatives now import
the package. Native acceptance uses manifest BUILD and standalone execution;
a bare source compile cannot resolve manifest dependencies.

## Historical focused scalar-append acceptance (before package migration)

The revised scalar extension passed source checking (40 functions) and the
focused `owned_utf8_text` regression on source tree
`3e9726a9cc661d63151251c4f612c1df344c48d4`, based on commit
`5ebc252b6af7819195fab8537d38ba852199b104`. The ordinary test run passed two
tests with one native test ignored. The explicit LLVM 19.1.7 run including
ignored tests passed all three tests. The regression asserts exact result-mode
stdout `42\n`, exit 0, and empty stderr for both interpreter and standalone
native execution, and checks private-field and moved-owner rejection.
Scoped `rustfmt --check --edition 2021 tests/owned_utf8_text.rs` and
`cargo clippy --locked --test owned_utf8_text -- -D warnings` also passed.
Only this README changed after validation; executable sources and regression
remain byte-identical. No full repository suite, PR-head CI, allocation-failure
injection, or general R2 completion is claimed.

The first candidate was rejected at check time with E0305 for direct ByteBuf
field indexing. The source-only repair compares through a private borrowed
slice helper, without compiler or runtime changes.

Existing checks -1..-33 remain unchanged. New partitioned checks -34..-57
exercise U+0000, U+007F, U+0080, U+07FF, U+0800, U+D7FF, U+E000, U+FFFF,
U+10000, and U+10FFFF; explicit negative, surrogate, above-maximum and
i32-extreme refusals; and append after refusal. Each successful append checks
fixed bytes, byte length, scalar count and validation of the equal expected
bytes. Each refusal checks exact original bytes, length and scalar count.
Truncation inside and at appended scalar boundaries is also checked.
Recoverable growth failures are forwarded but not forced. Independent Python
arithmetic/host-codec checks exhaustively agree for all 1,112,064 Unicode
scalars and reject all 2,048 surrogates; those checks are fixture oracles,
separate from the Oxid regression acceptance.

## Prior focused acceptance (before scalar extension)

The pre-extension partitioned client passed source checking (32 functions), interpreter
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
oxid build --manifest-path examples/projects/owned_utf8_text/oxid.toml --entry-mode=result
examples/projects/owned_utf8_text/.oxid/typed-preview/result/owned-utf8-text-prototype
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

## Path-package integration acceptance

The path-dependency migration passed CHECK (44 functions), interpreter
execution, and manifest BUILD followed by standalone native execution with
LLVM 19.1.7 on Linux x86_64. The focused Rust regression target passed two
ordinary tests and one native test, with scoped clippy passing. The final
Rust formatting change is mechanical. Original pre-migration results above
remain historical; no full repository or release qualification pass is claimed.
