# owned_text_slice: client contracts for `text::slice`

Pure Result-mode client of the ordinary path package `packages/utf8_text`
(alias `text`). It consumes `SliceResult` (`Ok(Text)`, `InvalidRange`,
`CapacityOverflow`, `AllocationFailed`, `MemoryLimit`) and
`slice(text: &Text, start: i32, end: i32) -> SliceResult`, together with the
existing `from_bytes`, `new_text`, `byte_len`, `scalar_count`, `equals_utf8`,
`append_utf8`, `truncate` and `into_bytes`. It does no I/O, and `main` returns
42 only when every contract passes.

The contract is a half-open byte range: success requires
`0 <= start <= end <= byte_len` with both ends on UTF-8 scalar boundaries, and
returns an independent owned copy while the original Text is unchanged. An
empty range succeeds only on a boundary. No zero-copy or grapheme claims are
made.

Fixed corpora, written as raw byte arrays:

| Corpus | Bytes | Scalar boundaries |
| --- | --- | --- |
| mixed: `A`, NUL, U+00A2, U+20AC, U+1F600, `z` | 12 | 0 1 2 4 7 11 12 |
| ascii_nul: `O x NUL i d` | 5 | every offset 0..5 |

Every expected slice is a separate hand-written array; nothing is derived from
the package result or by slicing the corpus. The `test_` cases cover the full
range, prefix/middle/suffix ranges, each scalar width alone, ASCII and NUL
subranges, empty ranges on boundaries (including an empty Text), start and end
inside each multi-byte scalar (offsets 3, 5, 6, 8, 9, 10), empty and both-end
interior ranges, negative bounds (including i32 minimum), reversed bounds, and
bounds past the end (including i32 maximum). Successful results are consumed
with `into_bytes` and compared byte for byte; changing or truncating the
returned Text or its recovered bytes, or truncating the original afterwards,
leaves the other side unchanged.

No capacity or fuel pressure and no allocation fault injection.

```sh
oxid check --manifest-path examples/projects/owned_text_slice/oxid.toml
oxid test --manifest-path examples/projects/owned_text_slice/oxid.toml
oxid run --manifest-path examples/projects/owned_text_slice/oxid.toml --engine=interpreter --entry-mode=result
```

The Rust acceptance is `tests/owned_text_slice.rs`. Its two native tests are
ignored by default. Set `OXID_TEST_LLVM_BIN` to an LLVM 19.1.7 bin directory
and explicitly enable ignored tests:

```sh
OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --locked --test owned_text_slice -- --include-ignored
```

## Focused validation

Package CHECK passed with 23 functions and client CHECK with 47 functions.
Ordinary acceptance passed two tests with two native tests ignored. With
LLVM 19.1.7, all four passed, including fourteen interpreter contracts,
fourteen native contracts, and a standalone executable run after removing
the copied sources, with cwd `/` and an empty environment. The existing
`owned_utf8_text` regression target also passed all three tests. Rustfmt,
scoped Clippy with warnings denied and typed-preview format checks passed.

The passing harness asserts exact `42\n`, exit 0 and empty stderr for the
Result and standalone paths, and the exact fourteen-case TEST summary.
Successful child captures are asserted in memory rather than separately
retained in the delivered logs. These are focused results; no full-suite,
capacity/fuel, allocation-failure or broader process qualification is claimed.
